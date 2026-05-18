use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::AppError;
use crate::AppState;
use crate::DEMO_FILES;

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
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return demo_nodes(rel).ok_or(AppError::NotFound),
    };

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

fn demo_nodes(rel: &str) -> Option<Vec<TreeNode>> {
    let rel = rel.trim_matches('/');
    let prefix = if rel.is_empty() {
        String::new()
    } else {
        format!("{rel}/")
    };
    let mut dirs = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut matched = false;

    for (path, _) in DEMO_FILES {
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        matched = true;
        if let Some((dir, _)) = rest.split_once('/') {
            dirs.insert(dir.to_string());
        } else if !rest.is_empty() {
            files.insert(rest.to_string());
        }
    }

    if !matched {
        return None;
    }

    let mut nodes = Vec::with_capacity(dirs.len() + files.len());
    for name in dirs {
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        nodes.push(TreeNode::Dir {
            name,
            rel_path: child_rel.clone(),
            children: demo_nodes(&child_rel).unwrap_or_default(),
        });
    }
    for name in files {
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        nodes.push(TreeNode::File {
            name,
            url_path: format!("/{}", encode_path_segments(&child_rel)),
        });
    }

    Some(nodes)
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

    Some(root.join(trimmed))
}
