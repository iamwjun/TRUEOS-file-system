mod html;
mod tree;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::env;

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
pub(crate) const DEMO_FILES: [(&str, &str); 10] = [
    (
        "demo-data/README.txt",
        "TRUEOS file-system demo data\n\nThis directory is created by the blueprint at startup.\n",
    ),
    (
        "demo-data/docs/intro.md",
        "# Demo Files\n\nThe file-system blueprint can serve direct files and tree pages.\n",
    ),
    (
        "demo-data/docs/routes.md",
        "# Routes\n\n/ and /tree render the tree. Direct paths serve files through tower-http.\n",
    ),
    (
        "demo-data/notes/todo.txt",
        "- verify tree rendering\n- open a direct file URL\n- test nested folders\n",
    ),
    (
        "demo-data/notes/status.txt",
        "status=seeded\nservice=file-system\ntransport=http\n",
    ),
    (
        "demo-data/assets/index.html",
        "<!doctype html><title>TRUEOS demo</title><h1>TRUEOS file-system demo</h1>\n",
    ),
    (
        "demo-data/assets/style.css",
        "body { font-family: sans-serif; margin: 2rem; }\nh1 { color: #2457d6; }\n",
    ),
    (
        "demo-data/logs/boot.log",
        "demo: blueprint startup seed complete\n",
    ),
    (
        "demo-data/logs/http.log",
        "demo: awaiting requests on 0.0.0.0:54321\n",
    ),
    (
        "demo-data/data/sample.json",
        "{ \"name\": \"TRUEOS\", \"app\": \"file-system\", \"files\": 10 }\n",
    ),
];

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

    let root = normalize_root(root);

    if let Err(err) = seed_demo_data(&root).await {
        eprintln!("demo data seed failed under {}: {err}", root.display());
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

async fn seed_demo_data(root: &Path) -> tokio::io::Result<()> {
    for (relative, body) in DEMO_FILES {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            match tokio::fs::create_dir_all(parent.display().to_string()).await {
                Ok(()) => {}
                Err(err) if err.kind() == tokio::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }
        }
        tokio::fs::write(path.display().to_string(), body).await?;
    }

    Ok(())
}

fn normalize_root(root: PathBuf) -> PathBuf {
    let text = root.display().to_string();
    if text.trim().is_empty() {
        PathBuf::from(".")
    } else {
        root
    }
}

async fn tree_root(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    render_tree(&state, &state.root, "")
}

async fn tree_subdir(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Html<String>, AppError> {
    let dir = resolve_under_root(&state.root, &path).ok_or(AppError::NotFound)?;
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
