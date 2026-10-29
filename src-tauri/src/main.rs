// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn is_mcp_invocation() -> bool {
    std::env::args_os().nth(1).is_some_and(|arg| arg == "mcp")
}

fn main() {
    if is_mcp_invocation() {
        std::process::exit(saldo_lib::run_mcp());
    }
    saldo_lib::run()
}
