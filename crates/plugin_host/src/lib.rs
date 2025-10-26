//! Wasmtime runner + WASI sandbox (REFACTOR): minimal runner with deny-by-default posture.
//! - Engine with fuel enabled; per-invoke fuel budget to bound CPU (default: 1M units).
//! - Epoch-based timeout to bound wall time (default: 500 ms per invoke).
//! - WASI wired with no preopens/network (no ambient authority).
//! - Memory capped via Store limits (fail-closed defaults; default: 128 MiB).
//!
//! TODO(observability): add metrics/traces (plugin.invoke.ms, plugin.fuel.consumed, plugin.mem.bytes).

use std::sync::Arc;
use std::time::Duration;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tracing::{field, info_span};

#[cfg(feature = "otel")]
mod verify_metrics {
    use opentelemetry::metrics::{Counter, Histogram, Meter, Unit};
    use opentelemetry::{global, KeyValue};
    use std::sync::OnceLock;

    static INSTR: OnceLock<(Counter<u64>, Histogram<f64>)> = OnceLock::new();

    fn instruments() -> &'static (Counter<u64>, Histogram<f64>) {
        INSTR.get_or_init(|| {
            let meter: Meter = global::meter("plugin_host");
            let failures = meter
                .u64_counter("plugin.verify.failures")
                .with_description("Number of manifest verification failures")
                .init();
            let verify_ms = meter
                .f64_histogram("plugin.verify.ms")
                .with_description("Manifest verification duration (ms)")
                .with_unit(Unit::new("ms"))
                .init();
            (failures, verify_ms)
        })
    }

    pub fn inc_failure(error_code: &'static str) {
        let (c, _) = instruments();
        c.add(1, &[KeyValue::new("error_code", error_code)]);
    }

    pub fn observe_ms(ms: f64) {
        let (_, h) = instruments();
        h.record(ms, &[]);
    }
}

use wasmtime::{Config, Engine, Instance, Linker, Module, Store};
use wasmtime::{StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::preview1::wasi_snapshot_preview1::add_to_linker as add_wasi_to_linker;

use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

/// Errors from the plugin runner.
#[derive(Debug, Error)]
pub enum RunnerError {
    /// Compiling/loading a module failed.
    #[error("load failed: {0}")]
    LoadFailed(String),
    /// Invoking an exported function failed.
    #[error("invoke failed: {0}")]
    InvokeFailed(String),
}

/// Opaque handle for a loaded module (compiled via Wasmtime `Module`).
#[derive(Debug, Clone)]
pub struct ModuleHandle {
    module: Arc<Module>,
}

impl ModuleHandle {
    #[inline]
    fn new(module: Module) -> Self {
        Self { module: Arc::new(module) }
    }
}

/// Minimal Wasmtime-backed plugin runner holding a shared `Engine` and default limits.
#[derive(Clone)]
pub struct PluginRunner {
    engine: Arc<Engine>,
    memory_limit_bytes: usize,
    fuel_budget: u64,
    timeout_ms: u64,
}

impl Default for PluginRunner {
    fn default() -> Self {
        let mut cfg = Config::new();
        cfg.async_support(true);
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        let engine = Engine::new(&cfg).expect("engine config should be valid");
        Self {
            engine: Arc::new(engine),
            memory_limit_bytes: 128 * 1024 * 1024,
            fuel_budget: 1_000_000,
            timeout_ms: 500,
        }
    }
}

impl PluginRunner {
    /// Create a new runner instance with default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a runner with explicit limits (primarily for tests).
    ///
    /// # Panics
    /// Panics if the Wasmtime engine configuration is invalid.
    #[must_use]
    pub fn with_limits(memory_limit_bytes: usize) -> Self {
        let mut cfg = Config::new();
        cfg.async_support(true);
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        let engine = Engine::new(&cfg).expect("engine config should be valid");
        Self {
            engine: Arc::new(engine),
            memory_limit_bytes,
            fuel_budget: 1_000_000,
            timeout_ms: 500,
        }
    }

    /// Create a runner with explicit memory/fuel/timeout budgets (primarily for tests).
    ///
    /// # Panics
    /// Panics if the Wasmtime engine configuration is invalid.
    #[must_use]
    pub fn with_limits_and_budgets(
        memory_limit_bytes: usize,
        fuel_budget: u64,
        timeout_ms: u64,
    ) -> Self {
        let mut cfg = Config::new();
        cfg.async_support(true);
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        let engine = Engine::new(&cfg).expect("engine config should be valid");
        Self { engine: Arc::new(engine), memory_limit_bytes, fuel_budget, timeout_ms }
    }

    /// Compile WASM bytes into a `Module` and return a handle.
    ///
    /// # Errors
    /// Returns [`RunnerError::LoadFailed`] when compilation fails.
    pub fn load_module(&self, wasm: &[u8]) -> Result<ModuleHandle, RunnerError> {
        Module::new(&self.engine, wasm)
            .map(ModuleHandle::new)
            .map_err(|e| RunnerError::LoadFailed(e.to_string()))
    }

    /// Instantiate the module and invoke a typed export: (i32, i32) -> i32.
    ///
    /// # Errors
    /// Returns [`RunnerError::InvokeFailed`] when instantiation, lookup, or call fails,
    /// including resource budget violations (fuel exhaustion or timeout via epoch interruption).
    pub fn invoke_i32_2(
        &self,
        module: &ModuleHandle,
        func: &str,
        a: i32,
        b: i32,
    ) -> Result<i32, RunnerError> {
        // Store state carries WASI context and resource limits; limiter returns a mutable
        // reference to the limits enabling Wasmtime to enforce them.
        struct StoreState {
            wasi: WasiP1Ctx,
            limits: StoreLimits,
        }

        let wasi = WasiCtxBuilder::new().build_p1();
        let limits = StoreLimitsBuilder::new().memory_size(self.memory_limit_bytes).build();
        let mut store: Store<StoreState> = Store::new(&self.engine, StoreState { wasi, limits });
        // Attach the limiter; Wasmtime will consult this to enforce memory/table/instance caps.
        store.limiter(|s| &mut s.limits);
        // Add fuel budget (CPU bound) and set epoch deadline for timeouts.
        store.set_fuel(self.fuel_budget).map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;
        store.set_epoch_deadline(1);
        let engine_for_timeout = self.engine.clone();
        let timeout_ms = self.timeout_ms;
        let _timeout_thr = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(timeout_ms));
            engine_for_timeout.increment_epoch();
        });

        let mut linker: Linker<StoreState> = Linker::new(&self.engine);
        add_wasi_to_linker(&mut linker, |s: &mut StoreState| &mut s.wasi)
            .map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;
        #[cfg(feature = "hostcalls")]
        {
            use std::str;
            linker
                .func_wrap(
                    "env",
                    "host_log",
                    |mut caller: wasmtime::Caller<'_, StoreState>, ptr: i32, len: i32| -> i32 {
                        let Some(wasmtime::Extern::Memory(mem)) = caller.get_export("memory")
                        else {
                            return -1;
                        };
                        let Ok(ptr) = usize::try_from(ptr) else {
                            return -1;
                        };
                        let Ok(len) = usize::try_from(len) else {
                            return -1;
                        };
                        let data = mem.data(&caller);
                        let end = ptr.saturating_add(len);
                        if end > data.len() {
                            return -1;
                        }
                        str::from_utf8(&data[ptr..end]).map_or(-1, |s| {
                            eprintln!("[plugin] {s}");
                            0
                        })
                    },
                )
                .map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;
        }

        let instance: Instance =
            pollster::block_on(linker.instantiate_async(&mut store, &module.module))
                .map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;

        let func_typed = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, func)
            .map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;

        match pollster::block_on(func_typed.call_async(&mut store, (a, b))) {
            Ok(v) => Ok(v),
            Err(e) => {
                let fuel = store.get_fuel().ok();
                let suffix = match fuel {
                    Some(0) => " (fuel exhausted)".to_string(),
                    _ => " (timeout/epoch interruption)".to_string(),
                };
                Err(RunnerError::InvokeFailed(format!("{e}{suffix}")))
            }
        }
    }
}

/// Plugin manifest describing the WASM module and supply-chain metadata.
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Human-readable plugin name (informational only).
    pub name: String,
    /// Semantic version of the plugin (informational only).
    pub version: String,
    /// Hex-encoded SHA-256 of the WASM bytes (digest pinning, lowercase preferred).
    pub wasm_digest: String,
    /// Base64-encoded signature or Sigstore bundle material. None => unsigned.
    pub signature: Option<String>,
    /// Reference to SBOM (e.g., filename or digest). None => missing per policy.
    pub sbom_ref: Option<String>,
}

/// Verification errors for plugin manifests (fail-closed by default).
///
/// Stable `error_code` strings (used in spans/metrics):
/// - `missing_signature`
/// - `missing_sbom`
/// - `digest_mismatch`
/// - `invalid_signature`
/// - `invalid_digest_format`
/// - `oversized_signature`
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum VerificationError {
    /// Signature is required but missing (`require_signed_plugins=true`).
    #[error("manifest missing signature")]
    MissingSignature,
    /// SBOM reference is required but missing (`require_signed_plugins=true`).
    #[error("manifest missing SBOM reference")]
    MissingSbom,
    /// The `manifest.wasm_digest` is not exactly 64 hex chars after trim+lowercase.
    #[error("invalid digest format")]
    InvalidDigestFormat,
    /// WASM digest did not match `manifest.wasm_digest`.
    #[error("digest mismatch")]
    DigestMismatch,
    /// Signature present but exceeds size cap (16 KiB after trim).
    #[error("oversized signature")]
    OversizedSignature,
    /// Signature present but failed offline verification/decoding.
    #[error("invalid signature")]
    InvalidSignature,
    /// Other error category.
    #[error("{0}")]
    Other(String),
}

/// Offline verifier (no network). Policy: `require_signed_plugins=true` by default.
impl From<base64::DecodeError> for VerificationError {
    fn from(_e: base64::DecodeError) -> Self {
        Self::InvalidSignature
    }
}

const MAX_SIG_LEN: usize = 16 * 1024;

fn normalize_and_validate_digest(s: &str) -> Result<[u8; 32], VerificationError> {
    let norm = s.trim().to_ascii_lowercase();
    if norm.len() != 64 || !norm.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(VerificationError::InvalidDigestFormat);
    }
    let bytes = hex::decode(&norm).map_err(|_| VerificationError::InvalidDigestFormat)?;
    if bytes.len() != 32 {
        return Err(VerificationError::InvalidDigestFormat);
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn validate_signature_size(s: &str) -> Result<(), VerificationError> {
    let len = s.trim().len();
    if len > MAX_SIG_LEN {
        return Err(VerificationError::OversizedSignature);
    }
    Ok(())
}

/// Offline manifest verifier (deterministic, fail-closed).
#[derive(Debug, Clone)]
pub struct ManifestVerifier {
    /// When true, signatures and SBOM references are required; deny on any error.
    pub require_signed_plugins: bool,
}

impl Default for ManifestVerifier {
    fn default() -> Self {
        Self { require_signed_plugins: true }
    }
}

impl ManifestVerifier {
    /// Construct a verifier with default fail-closed policy (require signatures).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Verify manifest against provided WASM bytes.
    ///
    /// Deterministic, offline-only; no network I/O or wall-clock dependencies.
    ///
    /// # Errors
    /// Returns:
    /// - `VerificationError::MissingSignature` when a signature is required but not present.
    /// - `VerificationError::MissingSbom` when SBOM reference is required but missing.
    /// - `VerificationError::DigestMismatch` when the WASM digest does not match the manifest.
    /// - `VerificationError::InvalidSignature` when signature decoding/verification fails.
    pub fn verify(&self, manifest: &PluginManifest, wasm: &[u8]) -> Result<(), VerificationError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine as _;
        use sha2::Digest as _;

        // Observability span (no control-path changes).
        let span =
            info_span!("agent.plugin.verify", result = field::Empty, error_code = field::Empty);
        let _g = span.enter();
        #[cfg(feature = "otel")]
        let __start = std::time::Instant::now();

        // Policy gates first: require signature and SBOM if configured.
        if self.require_signed_plugins {
            if manifest.signature.is_none() {
                span.record("result", "error");
                span.record("error_code", field::display("missing_signature"));
                #[cfg(feature = "otel")]
                {
                    verify_metrics::inc_failure("missing_signature");
                    verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
                }
                return Err(VerificationError::MissingSignature);
            }
            if manifest.sbom_ref.is_none() {
                span.record("result", "error");
                span.record("error_code", field::display("missing_sbom"));
                #[cfg(feature = "otel")]
                {
                    verify_metrics::inc_failure("missing_sbom");
                    verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
                }
                return Err(VerificationError::MissingSbom);
            }
        }

        // Validate manifest digest format and decode expected digest bytes.
        let expected = match normalize_and_validate_digest(&manifest.wasm_digest) {
            Ok(b) => b,
            Err(e) => {
                span.record("result", "error");
                span.record("error_code", field::display("invalid_digest_format"));
                #[cfg(feature = "otel")]
                {
                    verify_metrics::inc_failure("invalid_digest_format");
                    verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
                }
                return Err(e);
            }
        };

        // Digest pinning: sha256(WASM) must equal manifest.wasm_digest (hex, case-insensitive).
        let mut hasher = sha2::Sha256::new();
        hasher.update(wasm);
        let actual_vec = hasher.finalize();
        let mut actual = [0u8; 32];
        actual.copy_from_slice(&actual_vec);
        if !bool::from(actual.ct_eq(&expected)) {
            span.record("result", "error");
            span.record("error_code", field::display("digest_mismatch"));
            #[cfg(feature = "otel")]
            {
                verify_metrics::inc_failure("digest_mismatch");
                verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
            }
            return Err(VerificationError::DigestMismatch);
        }

        // Offline signature verification: for now, require base64-encoded material and
        // return InvalidSignature if decoding or offline verification fails. This remains
        // offline and deterministic; network is not used.
        if let Some(sig) = &manifest.signature {
            let s = sig.trim();
            if let Err(e) = validate_signature_size(s) {
                span.record("result", "error");
                span.record("error_code", field::display("oversized_signature"));
                #[cfg(feature = "otel")]
                {
                    verify_metrics::inc_failure("oversized_signature");
                    verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
                }
                return Err(e);
            }
            if STANDARD.decode(s).is_err() {
                span.record("result", "error");
                span.record("error_code", field::display("invalid_signature"));
                #[cfg(feature = "otel")]
                {
                    verify_metrics::inc_failure("invalid_signature");
                    verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
                }
                return Err(VerificationError::InvalidSignature);
            }
            // TODO(SEC-04 follow-up): integrate sigstore offline verification against a pinned trust root/bundle.
            span.record("result", "error");
            span.record("error_code", field::display("invalid_signature"));
            #[cfg(feature = "otel")]
            {
                verify_metrics::inc_failure("invalid_signature");
                verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
            }
            return Err(VerificationError::InvalidSignature);
        }

        span.record("result", "ok");
        #[cfg(feature = "otel")]
        verify_metrics::observe_ms(__start.elapsed().as_secs_f64() * 1000.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_export_returns_error() {
        // (module (func (export "add") (param i32 i32) (result i32) local.get 0 local.get 1 i32.add))
        let wat = r#"(module (func (export "add") (param i32 i32) (result i32)
            local.get 0 local.get 1 i32.add))"#;
        let wasm = wat::parse_str(wat).expect("WAT -> WASM should succeed");
        let runner = PluginRunner::new();
        let handle = runner.load_module(&wasm).expect("load module");
        let err = runner.invoke_i32_2(&handle, "missing", 1, 2).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("invoke failed"));
    }

    #[test]
    fn memory_limit_exceeded_returns_error() {
        // Module exports a memory (1 page = 64KiB) and grows it by 1 page on call.
        let wat = r#"(module
            (memory (export "mem") 1)
            (func (export "grow") (param i32 i32) (result i32)
              local.get 0
              drop
              local.get 1
              drop
              i32.const 1
              memory.grow))"#;
        let wasm = wat::parse_str(wat).expect("WAT -> WASM should succeed");
        // Set limit to 64KiB; growing by one page should exceed the cap and error.
        let runner = PluginRunner::with_limits(64 * 1024);
        let handle = runner.load_module(&wasm).expect("load module");
        let res =
            runner.invoke_i32_2(&handle, "grow", 0, 0).expect("call should succeed or return -1");
        assert_eq!(res, -1, "memory.grow should be denied by limits and return -1");
    }

    #[test]
    fn fuel_exhaustion_returns_error() {
        // Infinite loop to burn fuel; should trap when fuel is exhausted.
        let wat = r#"(module
            (func (export "spin") (param i32 i32) (result i32)
              loop
                local.get 0
                drop
                local.get 1
                drop
                br 0
              end
              i32.const 0))"#;
        let wasm = wat::parse_str(wat).expect("WAT -> WASM should succeed");
        let runner = PluginRunner::with_limits_and_budgets(128 * 1024 * 1024, 1_000, 5_000);
        let handle = runner.load_module(&wasm).expect("load module");
        let err = runner.invoke_i32_2(&handle, "spin", 0, 0).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("fuel") || msg.to_lowercase().contains("exhaust"),
            "expected fuel exhaustion error, got: {msg}"
        );
    }

    #[test]
    fn timeout_exceeded_returns_error() {
        // Infinite loop; with large fuel but small timeout, should hit epoch interruption.
        let wat = r#"(module
            (func (export "spin") (param i32 i32) (result i32)
              loop
                br 0
              end
              i32.const 0))"#;
        let wasm = wat::parse_str(wat).expect("WAT -> WASM should succeed");
        let runner =
            PluginRunner::with_limits_and_budgets(128 * 1024 * 1024, 1_000_000_000_000, 100);
        let handle = runner.load_module(&wasm).expect("load module");
        let err = runner.invoke_i32_2(&handle, "spin", 0, 0).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("interrupt") || msg.to_lowercase().contains("epoch"),
            "expected timeout/epoch interruption, got: {msg}"
        );
    }

    #[cfg(feature = "hostcalls")]
    #[test]
    fn hostcall_invalid_bounds_returns_error() {
        // Calls host_log with out-of-bounds pointer/len; expect -1 result.
        let wat = r#"(module
            (import "env" "host_log" (func $log (param i32 i32) (result i32)))
            (memory (export "memory") 1)
            (func (export "bad") (param i32 i32) (result i32)
              i32.const 100000
              i32.const 10
              call $log))"#;
        let wasm = wat::parse_str(wat).expect("WAT -> WASM should succeed");
        let runner = PluginRunner::new();
        let handle = runner.load_module(&wasm).expect("load module");
        let res = runner.invoke_i32_2(&handle, "bad", 0, 0).expect("call should return -1");
        assert_eq!(res, -1);
    }
}
