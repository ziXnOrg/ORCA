//! Wasmtime runner skeleton + hostcalls (RED): minimal API stubs for T-6a-E3-PH-03.
//! Implements a `PluginRunner` with unimplemented methods to drive TDD.

use thiserror::Error;

/// Errors from the plugin runner.
#[derive(Debug, Error)]
pub enum RunnerError {
    /// Placeholder until GREEN: all methods return this in RED phase.
    #[error("unimplemented")]
    Unimplemented,
    /// Loading a module failed.
    #[error("load failed: {0}")]
    LoadFailed(String),
    /// Invoking an exported function failed.
    #[error("invoke failed: {0}")]
    InvokeFailed(String),
}

/// Opaque handle for a loaded module.
#[derive(Debug, Clone)]
pub struct ModuleHandle {
    _private: (),
}

/// Minimal Wasmtime-backed plugin runner (stubs in RED phase).
#[derive(Debug, Default)]
pub struct PluginRunner;

impl PluginRunner {
    /// Create a new runner instance.
    ///
    /// This constructor performs no I/O or allocation.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Load a wasm module and return an opaque handle.
    ///
    /// # Errors
    /// Returns [`RunnerError::Unimplemented`] in RED phase.
    #[allow(clippy::missing_const_for_fn)]
    pub fn load_module(&self, _wasm: &[u8]) -> Result<ModuleHandle, RunnerError> {
        Err(RunnerError::Unimplemented)
    }

    /// Invoke an exported function taking two i32 and returning i32.
    ///
    /// # Errors
    /// Returns [`RunnerError::Unimplemented`] in RED phase.
    #[allow(clippy::missing_const_for_fn)]
    pub fn invoke_i32_2(
        &self,
        _module: &ModuleHandle,
        _func: &str,
        _a: i32,
        _b: i32,
    ) -> Result<i32, RunnerError> {
        Err(RunnerError::Unimplemented)
    }
}
