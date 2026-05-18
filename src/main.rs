mod html;
mod tree;

use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::services::ServeDir;

use tree::{resolve_under_root, scan_dir};

const PORT: u16 = 54321;

#[derive(Clone)]
struct AppState {
    root: PathBuf,
}

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

    let state = AppState { root: root.clone() };
    let serve_dir = ServeDir::new(&root).append_index_html_on_directories(true);

    let app = Router::new()
        .route("/", get(tree_root))
        .route("/tree", get(tree_root))
        .route("/tree/{*path}", get(tree_subdir))
        .with_state(state)
        .fallback_service(serve_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to bind {addr}: {e}");
            std::process::exit(1);
        });

    println!("Serving {} at http://{addr}/", root.display());
    println!("File tree: http://{addr}/tree");

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| {
            eprintln!("server error: {e}");
            std::process::exit(1);
        });
}

async fn tree_root(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    render_tree(&state, &state.root, "")
}

async fn tree_subdir(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Html<String>, AppError> {
    let dir = resolve_under_root(&state.root, &path).ok_or(AppError::NotFound)?;
    if !dir.is_dir() {
        return Err(AppError::NotFound);
    }
    render_tree(&state, &dir, &path)
}

fn render_tree(state: &AppState, dir: &Path, rel: &str) -> Result<Html<String>, AppError> {
    let nodes = scan_dir(state, dir, rel)?;
    Ok(Html(html::render_tree_page(
        &state.root,
        dir,
        rel,
        &nodes,
    )))
}

pub(crate) enum AppError {
    NotFound,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }
}
