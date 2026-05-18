use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use tower_http::services::ServeDir;

const PORT: u16 = 54321;

#[tokio::main]
async fn main() {
    let root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let root = match root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("invalid root directory {}: {e}", root.display());
            std::process::exit(1);
        }
    };

    if !root.is_dir() {
        eprintln!("root is not a directory: {}", root.display());
        std::process::exit(1);
    }

    let serve_dir = ServeDir::new(&root).append_index_html_on_directories(true);
    let app = Router::new().fallback_service(serve_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to bind {addr}: {e}");
            std::process::exit(1);
        });

    println!("Serving {} at http://{addr}/", root.display());

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| {
            eprintln!("server error: {e}");
            std::process::exit(1);
        });
}
