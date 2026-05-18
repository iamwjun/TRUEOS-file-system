# TRUEOS File System

A lightweight static file server written in Rust. It serves files from a configurable root directory over HTTP and provides an HTML file-tree browser for navigation.

Built with [Axum](https://github.com/tokio-rs/axum) and [tower-http](https://github.com/tower-rs/tower-http).

## Features

- **Static file serving** — download or open files under the root directory
- **HTML file tree** — browse the full directory hierarchy in the browser
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

## Routes

| Method | Path | Handler |
|--------|------|---------|
| `GET` | `/`, `/tree` | HTML file tree for the root |
| `GET` | `/tree/*path` | HTML file tree for a subdirectory |
| `GET` | `/*` (fallback) | Static file via `tower-http` `ServeDir` |

Notes:

- Hidden files and directories (names starting with `.`) are omitted from the tree view.
- Directories without `index.html` return the tree page when accessed via `/tree/...`; the static fallback serves `index.html` when present.

## Project layout

```
TRUEOS-file-system/
├── Cargo.toml
├── src/
│   ├── main.rs    # Server setup, routing, HTTP handlers
│   ├── tree.rs    # Directory scanning, path encoding, traversal checks
│   └── html.rs    # HTML page rendering and styles
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
