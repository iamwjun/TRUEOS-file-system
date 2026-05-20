# TRUEOS File System

A lightweight static file server written in Rust. It serves files from a configurable root directory over HTTP and provides an HTML file-tree browser for navigation.

Built with [Axum](https://github.com/tokio-rs/axum) and [tower-http](https://github.com/tower-rs/tower-http).

## Features

- **Static file serving** — download or open files under the root directory
- **HTML file tree** — browse the full directory hierarchy in the browser
- **Shared UI stylesheet** — all visual tokens and component styles are served from one reusable CSS file
- **Asynchronous job queue** — move, delete, upload, and download preparation run through a background worker
- **Job status pages** — every queued operation gets a dedicated status page and result link when applicable
- **Configurable root** — pass any directory as the serve root via CLI
- **Path safety** — requests are canonicalized and must stay within the root (blocks `..` traversal)
- **Dark mode** — UI follows system `prefers-color-scheme`

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.85+ (edition 2024)

## Quick start

```bash
# Clone and enter the repo
git clone https://github.com/iamwjun/TRUEOS-file-system.git
cd TRUEOS-file-system

# Run with the bundled example directory
cargo run -- example
```

Then open:

| URL | Description |
|-----|-------------|
| http://127.0.0.1:54321/ | File tree (root) |
| http://127.0.0.1:54321/tree | Same as `/` |
| http://127.0.0.1:54321/tree/subdir | File tree for a subdirectory |
| http://127.0.0.1:54321/jobs | Job queue overview page |
| http://127.0.0.1:54321/ui/style.css | Shared CSS asset |
| http://127.0.0.1:54321/file.txt | Direct file access |

The server listens on **port `54321`** (`0.0.0.0`).

## Usage

```bash
# Default root: current directory
cargo run

# Custom root directory
cargo run -- /path/to/files

# Release build
cargo build --release
./target/release/file-system /path/to/files
```

### CLI

| Argument | Default | Description |
|----------|---------|-------------|
| `[ROOT]` | `.` | Directory to serve (must exist and be a folder) |

## Rust API

The crate now exposes a reusable Rust API as `file_system`, so callers can submit file jobs without going through HTTP.

```rust
use std::time::Duration;

use file_system::{JobQueue, JobRequest, JobStatus};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queue = JobQueue::new("example");

    let upload = queue
        .enqueue(JobRequest::upload(
            "demo-data/uploads",
            "notes.txt",
            b"hello from rust".to_vec(),
        )?)
        .await?;

    let upload = queue
        .wait_for_terminal(upload.id, Duration::from_millis(50))
        .await
        .expect("job should exist");

    if upload.status == JobStatus::Succeeded {
        println!("upload result: {:?}", upload.result_path);
    }

    let move_job = queue
        .enqueue(JobRequest::move_path(
            "demo-data/uploads/notes.txt",
            "demo-data/archive/notes.txt",
        )?)
        .await?;

    println!("queued move job {}", move_job.id);
    Ok(())
}
```

Convenience methods are also available:

- `JobQueue::enqueue_move`
- `JobQueue::enqueue_delete`
- `JobQueue::enqueue_upload`
- `JobQueue::enqueue_download`
- `JobQueue::wait_for_terminal`

For a complete program, see `cargo run --example rust_api`.

## Routes

| Method | Path | Handler |
|--------|------|---------|
| `GET` | `/`, `/tree` | HTML file tree for the root |
| `GET` | `/tree/*path` | HTML file tree for a subdirectory |
| `GET` | `/ui/style.css` | Shared stylesheet for the UI |
| `GET` | `/jobs` | HTML job queue overview |
| `GET` | `/jobs/:id` | HTML job detail and status page |
| `POST` | `/jobs/move` | Enqueue a move job |
| `POST` | `/jobs/delete` | Enqueue a delete job |
| `POST` | `/jobs/upload` | Enqueue an upload job |
| `POST` | `/jobs/download` | Enqueue a staged download job |
| `GET` | `/*` (fallback) | Static file via `tower-http` `ServeDir` |

Notes:

- Hidden files and directories (names starting with `.`) are omitted from the tree view.
- Directories without `index.html` return the tree page when accessed via `/tree/...`; the static fallback serves `index.html` when present.
- Download jobs stage a copy under `/.job-downloads/...`, which stays hidden from the tree but remains directly servable.

## Project layout

```
TRUEOS-file-system/
├── assets/
│   └── ui.css     # Shared CSS tokens and component styles
├── Cargo.toml
├── examples/
│   └── rust_api.rs # Programmatic Rust API example
├── src/
│   ├── lib.rs      # Public Rust API exports
│   ├── main.rs    # Server setup, routing, HTTP handlers
│   ├── jobs.rs    # Asynchronous file operation queue and worker
│   ├── tree.rs    # Directory scanning, path encoding, traversal checks
│   └── html.rs    # HTML page rendering for tree and job pages
└── example/       # Sample files for local testing
```

## Development

```bash
# Build
cargo build

# Run tests (if added)
cargo test

# Format & lint
cargo fmt
cargo clippy
```

## Security

This is a **development-oriented** static server, not hardened for production:

- No authentication or access control
- Binds to all interfaces (`0.0.0.0`)
- Only serves content under the canonicalized root; path traversal via `..` is rejected

Do not expose it to untrusted networks without additional protection (reverse proxy, firewall, auth).

## License

See repository license file if present.
