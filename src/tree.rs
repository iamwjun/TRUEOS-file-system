use std::fs;
use std::path::{Path, PathBuf};

use crate::AppError;
use crate::AppState;

#[derive(Debug)]
pub enum TreeNode {
    Dir {
        name: String,
        rel_path: String,
        children: Vec<TreeNode>,
    },
    File {
        name: String,
        url_path: String,
    },
}

pub fn scan_dir(_state: &AppState, dir: &Path, rel: &str) -> Result<Vec<TreeNode>, AppError> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(dir).map_err(|_| AppError::NotFound)?;

    for entry in read_dir.flatten() {
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }

        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };

        if file_type.is_dir() {
            let children = scan_dir(_state, &entry.path(), &child_rel)?;
            entries.push(TreeNode::Dir {
                name,
                rel_path: child_rel,
                children,
            });
        } else if file_type.is_file() {
            entries.push(TreeNode::File {
                name,
                url_path: format!("/{}", encode_path_segments(&child_rel)),
            });
        }
    }

    entries.sort_by(|a, b| {
        let (a_dir, a_name) = node_sort_key(a);
        let (b_dir, b_name) = node_sort_key(b);
        b_dir
            .cmp(&a_dir)
            .then_with(|| a_name.cmp(b_name))
    });

    Ok(entries)
}

fn node_sort_key(node: &TreeNode) -> (bool, &str) {
    match node {
        TreeNode::Dir { name, .. } => (true, name.as_str()),
        TreeNode::File { name, .. } => (false, name.as_str()),
    }
}

pub fn encode_path_segments(path: &str) -> String {
    path.split('/').map(encode_segment).collect::<Vec<_>>().join("/")
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn resolve_under_root(root: &Path, request_path: &str) -> Option<PathBuf> {
    let trimmed = request_path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Some(root.to_path_buf());
    }
    if trimmed.split('/').any(|p| p == ".." || p.is_empty()) {
        return None;
    }

    let joined = root.join(trimmed);
    let canonical = joined.canonicalize().ok()?;
    if canonical.starts_with(root) {
        Some(canonical)
    } else {
        None
    }
}
