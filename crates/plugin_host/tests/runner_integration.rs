//! RED integration test for Wasmtime runner (T-6a-E3-PH-03)
//! Loads a minimal wasm module and invokes an exported function via the runner.

use plugin_host::PluginRunner;

#[test]
fn red_integration_invoke_add() {
    // Minimal module: (export "add") (param i32 i32) (result i32) => i32.add
    let wat = r#"(module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add))"#;

    let wasm = wat::parse_str(wat).expect("WAT to wasm should succeed");

    let runner = PluginRunner::new();
    let module = runner.load_module(&wasm).expect("load wasm module via wasmtime runner");

    let result = runner.invoke_i32_2(&module, "add", 2, 3).expect("invoke exported 'add' function");

    assert_eq!(result, 5);
}

#[cfg(feature = "hostcalls")]
#[test]
fn hostcall_log_integration() {
    // Writes "hi" at memory[0..2] and calls host_log; returns 42.
    let wat = r#"(module
      (import "env" "host_log" (func $log (param i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 0) "hi")
      (func (export "call_log") (param i32 i32) (result i32)
        local.get 0 drop
        local.get 1 drop
        i32.const 0
        i32.const 2
        call $log
        drop
        i32.const 42))"#;

    let wasm = wat::parse_str(wat).expect("WAT to wasm should succeed");

    let runner = PluginRunner::new();
    let module = runner.load_module(&wasm).expect("load wasm module");

    let result = runner.invoke_i32_2(&module, "call_log", 123, 456).expect("invoke call_log");
    assert_eq!(result, 42);
}
