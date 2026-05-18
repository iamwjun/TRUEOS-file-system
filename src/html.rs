use std::path::Path;

use crate::tree::{encode_path_segments, TreeNode};

const STYLES: &str = r#"
  :root {
    color-scheme: light dark;
    --bg: #f0f2f5;
    --surface: #ffffff;
    --surface-2: #f8f9fb;
    --border: #e4e7ec;
    --text: #101828;
    --muted: #667085;
    --accent: #444ce7;
    --accent-hover: #3538cd;
    --accent-soft: #eef4ff;
    --folder: #f79009;
    --folder-bg: #fffaeb;
    --file: #12b76a;
    --file-bg: #ecfdf3;
    --shadow: 0 1px 2px rgba(16, 24, 40, 0.06), 0 8px 24px rgba(16, 24, 40, 0.08);
    --radius: 12px;
    --radius-sm: 8px;
    --font: "SF Pro Text", "Segoe UI", system-ui, -apple-system, sans-serif;
    --mono: ui-monospace, "SF Mono", "Cascadia Code", monospace;
  }

  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #0c111d;
      --surface: #151b28;
      --surface-2: #1c2433;
      --border: #2a3447;
      --text: #f2f4f7;
      --muted: #98a2b3;
      --accent: #8098f9;
      --accent-hover: #a4bcfd;
      --accent-soft: #1d2939;
      --folder: #fdb022;
      --folder-bg: #422006;
      --file: #32d583;
      --file-bg: #053321;
      --shadow: 0 1px 2px rgba(0, 0, 0, 0.3), 0 12px 32px rgba(0, 0, 0, 0.45);
    }
  }

  * { box-sizing: border-box; }

  body {
    margin: 0;
    min-height: 100vh;
    font-family: var(--font);
    font-size: 15px;
    line-height: 1.5;
    color: var(--text);
    background:
      radial-gradient(ellipse 80% 50% at 50% -20%, rgba(68, 76, 231, 0.12), transparent),
      var(--bg);
    padding: 2rem 1rem 3rem;
  }

  .page {
    max-width: 52rem;
    margin: 0 auto;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    overflow: hidden;
  }

  .header {
    padding: 1.5rem 1.5rem 1.25rem;
    border-bottom: 1px solid var(--border);
    background: linear-gradient(180deg, var(--surface-2) 0%, var(--surface) 100%);
  }

  .header-top {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .logo {
    width: 2.25rem;
    height: 2.25rem;
    border-radius: 10px;
    background: linear-gradient(135deg, var(--accent) 0%, #7a5af8 100%);
    display: grid;
    place-items: center;
    color: #fff;
    font-size: 1.1rem;
    flex-shrink: 0;
  }

  h1 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .subtitle {
    margin: 0.15rem 0 0;
    font-size: 0.875rem;
    color: var(--muted);
  }

  .path-bar {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.625rem 0.75rem;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--mono);
    font-size: 0.8125rem;
    color: var(--muted);
    word-break: break-all;
    line-height: 1.45;
  }

  .path-bar::before {
    content: "⌁";
    flex-shrink: 0;
    color: var(--accent);
    font-family: var(--font);
    font-weight: 600;
  }

  .nav {
    padding: 0.875rem 1.5rem 0;
  }

  .nav a {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.4rem 0.85rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--accent);
    text-decoration: none;
    background: var(--accent-soft);
    border: 1px solid transparent;
    border-radius: 999px;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }

  .nav a:hover {
    color: var(--accent-hover);
    border-color: var(--border);
    background: var(--surface-2);
  }

  .tree-wrap {
    padding: 1rem 1.25rem 1.5rem;
  }

  .tree-stats {
    padding: 0 0.25rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  ul.tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  ul.tree ul.tree {
    margin-left: 1.125rem;
    padding-left: 0.875rem;
    border-left: 1px dashed var(--border);
  }

  li.tree-item {
    margin: 0.2rem 0;
  }

  .tree-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.6rem;
    border-radius: var(--radius-sm);
    transition: background 0.12s;
  }

  .tree-row:hover {
    background: var(--surface-2);
  }

  .icon {
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 6px;
    display: grid;
    place-items: center;
    font-size: 0.9rem;
    flex-shrink: 0;
  }

  .icon.folder {
    background: var(--folder-bg);
    color: var(--folder);
  }

  .icon.file {
    background: var(--file-bg);
    color: var(--file);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }

  li.file .name a {
    color: var(--text);
    text-decoration: none;
    transition: color 0.12s;
  }

  li.file .name a:hover {
    color: var(--accent);
  }

  .action {
    flex-shrink: 0;
    padding: 0.2rem 0.55rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--muted);
    text-decoration: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    opacity: 0;
    transition: opacity 0.12s, color 0.12s, border-color 0.12s;
  }

  .tree-row:hover .action {
    opacity: 1;
  }

  .action:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  .empty {
    padding: 2.5rem 1rem;
    text-align: center;
    color: var(--muted);
    font-size: 0.9375rem;
  }

  .empty-icon {
    font-size: 2rem;
    margin-bottom: 0.5rem;
    opacity: 0.5;
  }

  footer {
    margin-top: 1.25rem;
    text-align: center;
    font-size: 0.75rem;
    color: var(--muted);
  }
"#;

pub fn render_tree_page(root: &Path, current: &Path, rel: &str, nodes: &[TreeNode]) -> String {
    let title = if rel.is_empty() {
        root.display().to_string()
    } else {
        rel.to_string()
    };

    let (dir_count, file_count) = count_nodes(nodes);

    let body = if nodes.is_empty() {
        r#"<div class="empty"><div class="empty-icon">📂</div><p>This directory is empty</p></div>"#
            .to_string()
    } else {
        let mut tree = String::from("<ul class=\"tree\">\n");
        for node in nodes {
            render_node(&mut tree, node);
        }
        tree.push_str("</ul>\n");
        tree
    };

    let parent_link = if rel.is_empty() {
        String::new()
    } else {
        let parent = rel.rfind('/').map(|i| &rel[..i]).unwrap_or("");
        let href = if parent.is_empty() {
            "/tree".to_string()
        } else {
            format!("/tree/{}", encode_path_segments(parent))
        };
        format!(
            r#"<nav class="nav"><a href="{href}"><span aria-hidden="true">←</span> Parent directory</a></nav>"#
        )
    };

    let stats = if nodes.is_empty() {
        String::new()
    } else {
        format!("{dir_count} folders · {file_count} files")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title} · File tree</title>
  <style>{styles}</style>
</head>
<body>
  <div class="page">
    <div class="card">
      <header class="header">
        <div class="header-top">
          <div class="logo" aria-hidden="true">⬡</div>
          <div>
            <h1>File tree</h1>
            <p class="subtitle">Browse static files</p>
          </div>
        </div>
        <div class="path-bar">{root_display}</div>
      </header>
      {parent_link}
      <div class="tree-wrap">
        {stats_html}
        {body}
      </div>
    </div>
    <footer>TRUEOS File System</footer>
  </div>
</body>
</html>"#,
        title = escape(&title),
        styles = STYLES,
        root_display = escape(&current.display().to_string()),
        parent_link = parent_link,
        stats_html = if stats.is_empty() {
            String::new()
        } else {
            format!(r#"<p class="tree-stats">{stats}</p>"#)
        },
        body = body,
    )
}

fn count_nodes(nodes: &[TreeNode]) -> (usize, usize) {
    let mut dirs = 0;
    let mut files = 0;
    for node in nodes {
        match node {
            TreeNode::Dir { children, .. } => {
                dirs += 1;
                let (d, f) = count_nodes(children);
                dirs += d;
                files += f;
            }
            TreeNode::File { .. } => files += 1,
        }
    }
    (dirs, files)
}

fn render_node(out: &mut String, node: &TreeNode) {
    match node {
        TreeNode::Dir {
            name,
            rel_path,
            children,
        } => {
            let href = format!("/tree/{}", encode_path_segments(rel_path));
            out.push_str("<li class=\"tree-item dir\"><div class=\"tree-row\">");
            out.push_str("<span class=\"icon folder\" aria-hidden=\"true\">📁</span>");
            out.push_str("<span class=\"name\">");
            out.push_str(&escape(name));
            out.push_str("</span>");
            out.push_str("<a class=\"action\" href=\"");
            out.push_str(&href);
            out.push_str("\">Open</a></div>");
            if !children.is_empty() {
                out.push_str("\n<ul class=\"tree\">\n");
                for child in children {
                    render_node(out, child);
                }
                out.push_str("</ul>\n");
            }
            out.push_str("</li>\n");
        }
        TreeNode::File { name, url_path } => {
            out.push_str("<li class=\"tree-item file\"><div class=\"tree-row\">");
            out.push_str("<span class=\"icon file\" aria-hidden=\"true\">📄</span>");
            out.push_str("<span class=\"name\"><a href=\"");
            out.push_str(url_path);
            out.push_str("\">");
            out.push_str(&escape(name));
            out.push_str("</a></span></div></li>\n");
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
