mod html;
mod tree;

use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::{
    Form, Router,
    extract::{Multipart, Path as AxumPath, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_http::services::ServeDir;

use file_system::JobQueue;
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
    jobs: JobQueue,
}

#[derive(Deserialize)]
struct MoveJobForm {
    source: String,
    destination: String,
}

#[derive(Deserialize)]
struct DeleteJobForm {
    target: String,
}

#[derive(Deserialize)]
struct DownloadJobForm {
    source: String,
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

    let state = AppState {
        root: root.clone(),
        jobs: JobQueue::new(root.clone()),
    };
    let serve_dir = ServeDir::new(&root).append_index_html_on_directories(true);

    let app = Router::new()
        .route("/", get(tree_root))
        .route("/tree", get(tree_root))
        .route("/tree/{*path}", get(tree_subdir))
        .route("/ui/style.css", get(stylesheet))
        .route("/jobs", get(jobs_index))
        .route("/jobs/move", post(submit_move))
        .route("/jobs/delete", post(submit_delete))
        .route("/jobs/upload", post(submit_upload))
        .route("/jobs/download", post(submit_download))
        .route("/jobs/{id}", get(job_detail))
        .with_state(state)
        .fallback_service(serve_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|error| {
            eprintln!("failed to bind {addr}: {error}");
            std::process::exit(1);
        });

    println!("Serving {} at http://{addr}/", root.display());
    println!("File tree: http://{addr}/tree");
    println!("Job queue: http://{addr}/jobs");

    axum::serve(listener, app).await.unwrap_or_else(|error| {
        eprintln!("server error: {error}");
        std::process::exit(1);
    });
}

async fn seed_demo_data(root: &Path) -> tokio::io::Result<()> {
    for (relative, body) in DEMO_FILES {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            match tokio::fs::create_dir_all(parent).await {
                Ok(()) => {}
                Err(err) if err.kind() == tokio::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err),
            }
        }
        tokio::fs::write(path, body).await?;
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
    render_tree(&state, &state.root, "").await
}

async fn tree_subdir(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Html<String>, AppError> {
    let dir = resolve_under_root(&state.root, &path).ok_or(AppError::NotFound)?;
    render_tree(&state, &dir, &path).await
}

async fn render_tree(state: &AppState, dir: &Path, rel: &str) -> Result<Html<String>, AppError> {
    let nodes = scan_dir(dir, rel)?;
    let jobs = state.jobs.list(8).await;
    Ok(Html(html::render_tree_page(dir, rel, &nodes, &jobs)))
}

async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        html::stylesheet(),
    )
}

async fn jobs_index(State(state): State<AppState>) -> Html<String> {
    let jobs = state.jobs.list(64).await;
    Html(html::render_jobs_page(&state.root, &jobs))
}

async fn job_detail(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<u64>,
) -> Result<Html<String>, AppError> {
    let job = state.jobs.get(id).await.ok_or(AppError::NotFound)?;
    Ok(Html(html::render_job_page(&state.root, &job)))
}

async fn submit_move(
    State(state): State<AppState>,
    Form(form): Form<MoveJobForm>,
) -> Result<Redirect, AppError> {
    let job = state
        .jobs
        .enqueue_move(form.source, form.destination)
        .await
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    Ok(Redirect::to(&format!("/jobs/{}", job.id)))
}

async fn submit_delete(
    State(state): State<AppState>,
    Form(form): Form<DeleteJobForm>,
) -> Result<Redirect, AppError> {
    let job = state
        .jobs
        .enqueue_delete(form.target)
        .await
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    Ok(Redirect::to(&format!("/jobs/{}", job.id)))
}

async fn submit_download(
    State(state): State<AppState>,
    Form(form): Form<DownloadJobForm>,
) -> Result<Redirect, AppError> {
    let job = state
        .jobs
        .enqueue_download(form.source)
        .await
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    Ok(Redirect::to(&format!("/jobs/{}", job.id)))
}

async fn submit_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Redirect, AppError> {
    let mut directory = String::new();
    let mut filename = None;
    let mut bytes = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("invalid multipart body: {error}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "directory" => {
                directory = field.text().await.map_err(|error| {
                    AppError::BadRequest(format!("invalid target directory field: {error}"))
                })?;
            }
            "file" => {
                filename = field.file_name().map(|value| value.to_string());
                bytes = Some(field.bytes().await.map_err(|error| {
                    AppError::BadRequest(format!("failed to read uploaded file: {error}"))
                })?);
            }
            _ => {}
        }
    }

    let filename =
        filename.ok_or_else(|| AppError::BadRequest("missing uploaded file name".to_string()))?;
    let bytes =
        bytes.ok_or_else(|| AppError::BadRequest("missing uploaded file bytes".to_string()))?;

    let job = state
        .jobs
        .enqueue_upload(directory, filename, bytes.to_vec())
        .await
        .map_err(|error| AppError::BadRequest(error.to_string()))?;

    Ok(Redirect::to(&format!("/jobs/{}", job.id)))
}

pub(crate) enum AppError {
    NotFound,
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message).into_response(),
        }
    }
}
