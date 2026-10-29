mod open;
mod protocol;
mod tools;

/// Serves MCP on stdin and stdout until stdin closes and returns the process exit code. Logs go
/// to stderr: stdout carries only protocol lines.
pub fn serve_stdio(identifier: &str) -> i32 {
    // Tauri's `app_data_dir` is the same folder: the OS data directory joined with the
    // identifier.
    let Some(data_dir) = dirs::data_dir() else {
        eprintln!("saldo mcp: this system reports no application data folder");
        return 2;
    };
    let server = tools::Server::new(data_dir.join(identifier));
    match protocol::serve(std::io::stdin().lock(), std::io::stdout().lock(), &server) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("saldo mcp: {e}");
            1
        }
    }
}
