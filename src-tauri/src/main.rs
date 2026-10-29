// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // An exact first argument: a Finder launch passes none (or `-psn_…`), so it never picks
    // MCP mode by accident.
    if std::env::args_os().nth(1).is_some_and(|arg| arg == "mcp") {
        std::process::exit(saldo_lib::run_mcp());
    }
    saldo_lib::run()
}
