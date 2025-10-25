//! Wasmtime runner skeleton + hostcalls (GREEN): minimal Wasmtime-backed runner for T-6a-E3-PH-03.
//! Uses `wasmtime::{Engine, Module, Store, Linker}` to load and invoke exported functions.
//! Security posture: no ambient authority by default; WASI not linked yet (no imports required).

use std::sync::Arc;
use thiserror::Error;
use wasmtime::{Engine, Instance, Linker, Module, Store};

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

/// Minimal Wasmtime-backed plugin runner holding a shared `Engine`.
#[derive(Clone)]
pub struct PluginRunner {
    engine: Arc<Engine>,
}

impl Default for PluginRunner {
    fn default() -> Self {
        // Default Engine config: no special features; safe baseline.
        let engine = Engine::default();
        Self { engine: Arc::new(engine) }
    }
}

impl PluginRunner {
    /// Create a new runner instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    /// Returns [`RunnerError::InvokeFailed`] when instantiation, lookup, or call fails.
    pub fn invoke_i32_2(
        &self,
        module: &ModuleHandle,
        func: &str,
        a: i32,
        b: i32,
    ) -> Result<i32, RunnerError> {
        // No WASI/hostcalls are required for the test module (no imports),
        // so we use an empty `Store` data and a fresh `Linker`.
        let mut store: Store<()> = Store::new(&self.engine, ());
        let linker: Linker<()> = Linker::new(&self.engine);

        let instance: Instance = linker
            .instantiate(&mut store, &module.module)
            .map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;

        let func_typed = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, func)
            .map_err(|e| RunnerError::InvokeFailed(e.to_string()))?;

        func_typed.call(&mut store, (a, b)).map_err(|e| RunnerError::InvokeFailed(e.to_string()))
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
}
