mod open;
mod protocol;
mod tools;

use std::path::PathBuf;

use crate::features::ekasa::ekasa_api::LiveEkasaApi;

fn app_data_dir_for(identifier: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(identifier))
}

/// `resource_dir` is the bundle's resources folder, where PDFium is looked up first.
pub fn serve_stdio(identifier: &str, resource_dir: Option<PathBuf>) -> i32 {
    let Some(data_dir) = app_data_dir_for(identifier) else {
        eprintln!("saldo mcp: this system reports no application data folder");
        return 2;
    };
    let server = tools::Server::new(
        data_dir,
        tools::Outside {
            resource_dir,
            ekasa: Box::new(LiveEkasaApi),
        },
    );
    match protocol::serve(std::io::stdin().lock(), std::io::stdout().lock(), &server) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("saldo mcp: {e}");
            1
        }
    }
}
