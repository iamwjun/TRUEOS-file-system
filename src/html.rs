use std::path::Path;

use crate::tree::{TreeNode, encode_path_segments};
use file_system::jobs::{JobSnapshot, JobStatus};

pub const STYLE_PATH: &str = "/ui/style.css";

pub fn stylesheet() -> &'static str {
    include_str!("../assets/ui.css")
}

pub fn render_tree_page(
    current: &Path,
    rel: &str,
    nodes: &[TreeNode],
    jobs: &[JobSnapshot],
) -> String {
    let title = if rel.is_empty() {
        "File tree".to_string()
    } else {
        format!("{rel} · File tree")
    };
    let body = render_tree_body(current, rel, nodes, jobs);
    render_document(&title, &body, None)
}

pub fn render_jobs_page(root: &Path, jobs: &[JobSnapshot]) -> String {
    let body = format!(
        r#"
<div class="shell">
  <section class="header-card">
    <div class="header-card__top">
      <div class="brand">
        <div class="brand__mark" aria-hidden="true">⌘</div>
        <div>
          <p class="eyebrow">TRUEOS File System</p>
          <h1>Asynchronous Job Queue</h1>
          <p class="subtitle">Background file operations are queued and executed by the backend worker.</p>
        </div>
      </div>
      <div class="path-bar">{root_path}</div>
    </div>
    <div class="header-card__nav">
      <a class="pill-link" href="/tree">Back to tree</a>
    </div>
  </section>

  <section class="panel section-gap">
    <div class="panel__header">
      <h2>Recent jobs</h2>
      <p>Queue status across move, delete, upload, and download preparation tasks.</p>
    </div>
    <div class="panel__body">
      {jobs_html}
    </div>
  </section>

  <footer class="footer">Shared CSS and asynchronous worker enabled under S002.</footer>
</div>
"#,
        root_path = escape_html(&root.display().to_string()),
        jobs_html = render_jobs_list(jobs, true),
    );

    render_document("Job queue", &body, None)
}

pub fn render_job_page(root: &Path, job: &JobSnapshot) -> String {
    let body = format!(
        r#"
<div class="shell">
  <section class="detail-card">
    <div class="header-card__top">
      <div class="brand">
        <div class="brand__mark" aria-hidden="true">⚙</div>
        <div>
          <p class="eyebrow">Background task</p>
          <h1>Job #{job_id}</h1>
          <p class="subtitle">{summary}</p>
        </div>
      </div>
      <div class="path-bar">{root_path}</div>
    </div>
    <div class="header-card__nav">
      <a class="pill-link" href="/jobs">All jobs</a>
      <a class="action-link action-link--secondary" href="/tree">Back to tree</a>
    </div>
    <div class="panel__body">
      <span class="status-badge status-badge--{status_class}">{status_label}</span>
      <dl class="job-detail-grid">
        <dt>Operation</dt>
        <dd>{operation}</dd>
        <dt>Summary</dt>
        <dd>{summary}</dd>
        <dt>Detail</dt>
        <dd class="mono">{detail}</dd>
        <dt>Result path</dt>
        <dd>{result_path}</dd>
      </dl>
      {message_html}
    </div>
  </section>

  <footer class="footer">Active jobs auto-refresh every 2 seconds until completion.</footer>
</div>
"#,
        job_id = job.id,
        summary = escape_html(&job.summary),
        root_path = escape_html(&root.display().to_string()),
        status_class = job.status_class(),
        status_label = job.status_label(),
        operation = job.kind.label(),
        detail = escape_html(&job.detail),
        result_path = render_result_path(job),
        message_html = render_job_message(job),
    );

    let refresh = if job.status.is_terminal() {
        None
    } else {
        Some(2)
    };
    render_document(&format!("Job #{}", job.id), &body, refresh)
}

fn render_tree_body(current: &Path, rel: &str, nodes: &[TreeNode], jobs: &[JobSnapshot]) -> String {
    let title = if rel.is_empty() {
        "Root directory".to_string()
    } else {
        format!("Directory · {rel}")
    };
    let subtitle = if rel.is_empty() {
        "Browse files and dispatch background file operations from one place.".to_string()
    } else {
        format!("Current relative path: /{rel}")
    };
    let (dir_count, file_count) = count_nodes(nodes);

    format!(
        r#"
<div class="shell">
  <section class="header-card">
    <div class="header-card__top">
      <div class="brand">
        <div class="brand__mark" aria-hidden="true">⬡</div>
        <div>
          <p class="eyebrow">TRUEOS File System</p>
          <h1>{title}</h1>
          <p class="subtitle">{subtitle}</p>
        </div>
      </div>
      <div class="path-bar">{current_path}</div>
    </div>
    <div class="header-card__nav">
      {parent_link}
      <a class="action-link action-link--secondary" href="/jobs">View job queue</a>
    </div>
  </section>

  <div class="layout">
    <section class="panel">
      <div class="panel__header">
        <h2>File tree</h2>
        <p>Directories open through tree routes. Files keep direct static URLs.</p>
      </div>
      <div class="panel__body">
        {stats_html}
        {tree_html}
      </div>
    </section>

    <div class="stack">
      <section class="panel">
        <div class="panel__header">
          <h2>Dispatch operations</h2>
          <p>These forms enqueue background jobs instead of blocking the request path.</p>
        </div>
        <div class="panel__body">
          {forms_html}
        </div>
      </section>

      <section class="panel">
        <div class="panel__header">
          <h2>Recent jobs</h2>
          <p>The backend worker processes one job at a time from the queue.</p>
        </div>
        <div class="panel__body">
          {jobs_html}
        </div>
      </section>
    </div>
  </div>

  <footer class="footer">Shared visual tokens are served from one reusable CSS file.</footer>
</div>
"#,
        title = escape_html(&title),
        subtitle = escape_html(&subtitle),
        current_path = escape_html(&current.display().to_string()),
        parent_link = render_parent_link(rel),
        stats_html = render_tree_stats(dir_count, file_count),
        tree_html = render_tree(nodes),
        forms_html = render_job_forms(rel),
        jobs_html = render_jobs_list(jobs, false),
    )
}

fn render_tree(nodes: &[TreeNode]) -> String {
    if nodes.is_empty() {
        return r#"<div class="empty-state"><span class="empty-state__icon">📂</span><p>This directory is empty.</p></div>"#
            .to_string();
    }

    let mut tree = String::from(r#"<ul class="tree">"#);
    for node in nodes {
        render_node(&mut tree, node);
    }
    tree.push_str("</ul>");
    tree
}

fn render_job_forms(rel: &str) -> String {
    let upload_dir = escape_attr(rel);
    format!(
        r#"
<div class="stack">
  <form class="form-card form-grid" action="/jobs/move" method="post">
    <div>
      <h3>Move</h3>
      <p>Queue a rename or move operation inside the configured root.</p>
    </div>
    <div class="field">
      <label for="move-source">Source path</label>
      <input id="move-source" type="text" name="source" placeholder="demo-data/docs/intro.md" required>
    </div>
    <div class="field">
      <label for="move-destination">Destination path</label>
      <input id="move-destination" type="text" name="destination" placeholder="demo-data/archive/intro.md" required>
    </div>
    <button type="submit">Queue move job</button>
  </form>

  <form class="form-card form-grid" action="/jobs/delete" method="post">
    <div>
      <h3>Delete</h3>
      <p>Remove a file or directory through the background worker.</p>
    </div>
    <div class="field">
      <label for="delete-target">Target path</label>
      <input id="delete-target" type="text" name="target" placeholder="demo-data/logs/http.log" required>
    </div>
    <button type="submit">Queue delete job</button>
  </form>

  <form class="form-card form-grid" action="/jobs/upload" method="post" enctype="multipart/form-data">
    <div>
      <h3>Upload</h3>
      <p>Upload a file and let the queue write it into the selected directory.</p>
    </div>
    <div class="field">
      <label for="upload-directory">Target directory</label>
      <input id="upload-directory" type="text" name="directory" value="{upload_dir}" placeholder="demo-data/uploads">
    </div>
    <div class="field">
      <label for="upload-file">File</label>
      <input id="upload-file" type="file" name="file" required>
    </div>
    <button type="submit">Queue upload job</button>
  </form>

  <form class="form-card form-grid" action="/jobs/download" method="post">
    <div>
      <h3>Download</h3>
      <p>Prepare a staged download copy in the hidden job download area.</p>
    </div>
    <div class="field">
      <label for="download-source">Source file path</label>
      <input id="download-source" type="text" name="source" placeholder="demo-data/README.txt" required>
    </div>
    <button type="submit">Queue download job</button>
  </form>

  <p class="form-note">All paths are relative to the configured root. Parent traversal with <code>..</code> is rejected.</p>
</div>
"#,
        upload_dir = upload_dir,
    )
}

fn render_jobs_list(jobs: &[JobSnapshot], full_page: bool) -> String {
    if jobs.is_empty() {
        return r#"<p class="jobs-note">No jobs have been submitted yet.</p>"#.to_string();
    }

    let mut output = String::from(r#"<div class="jobs">"#);
    for job in jobs {
        output.push_str(&format!(
            r#"
<article class="job-item">
  <div class="job-item__top">
    <div class="job-item__meta">
      <strong>{operation}</strong>
      <span class="job-id">Job #{id}</span>
      <p class="job-summary">{summary}</p>
    </div>
    <span class="status-badge status-badge--{status_class}">{status_label}</span>
  </div>
  <div class="mono">{detail}</div>
  {message}
  <a class="action-link action-link--secondary" href="/jobs/{id}">Open details</a>
</article>
"#,
            operation = job.kind.label(),
            id = job.id,
            summary = escape_html(&job.summary),
            status_class = job.status_class(),
            status_label = job.status_label(),
            detail = escape_html(&job.detail),
            message = render_job_message(job),
        ));
    }
    if !full_page {
        output.push_str(r#"<a class="pill-link" href="/jobs">Open full queue view</a>"#);
    }
    output.push_str("</div>");
    output
}

fn render_job_message(job: &JobSnapshot) -> String {
    if let Some(error) = &job.error {
        return format!(
            r#"<div class="message message--error"><strong>Execution error:</strong> {}</div>"#,
            escape_html(error)
        );
    }

    if let Some(path) = &job.result_path {
        let safe_path = escape_attr(path);
        let label = escape_html(path);
        return format!(
            r#"<div class="message message--success"><strong>Result:</strong> <a href="{safe_path}">{label}</a></div>"#
        );
    }

    match job.status {
        JobStatus::Queued => {
            r#"<div class="message"><strong>Queued:</strong> waiting for the background worker.</div>"#
                .to_string()
        }
        JobStatus::Running => {
            r#"<div class="message"><strong>Running:</strong> the background worker is executing this task.</div>"#
                .to_string()
        }
        JobStatus::Succeeded => {
            r#"<div class="message message--success"><strong>Completed:</strong> no additional artifact path was produced.</div>"#
                .to_string()
        }
        JobStatus::Failed => String::new(),
    }
}

fn render_result_path(job: &JobSnapshot) -> String {
    if let Some(path) = &job.result_path {
        let safe_href = escape_attr(path);
        let safe_label = escape_html(path);
        format!(r#"<a class="mono" href="{safe_href}">{safe_label}</a>"#)
    } else {
        "<span class=\"mono\">n/a</span>".to_string()
    }
}

fn render_parent_link(rel: &str) -> String {
    if rel.is_empty() {
        return r#"<a class="pill-link" href="/tree">Root</a>"#.to_string();
    }

    let parent = rel.rfind('/').map(|index| &rel[..index]).unwrap_or("");
    let href = if parent.is_empty() {
        "/tree".to_string()
    } else {
        format!("/tree/{}", encode_path_segments(parent))
    };
    format!(r#"<a class="pill-link" href="{href}">Parent directory</a>"#)
}

fn render_tree_stats(dir_count: usize, file_count: usize) -> String {
    format!(r#"<p class="tree-stats">{dir_count} folders · {file_count} files</p>"#)
}

fn count_nodes(nodes: &[TreeNode]) -> (usize, usize) {
    let mut dirs = 0;
    let mut files = 0;
    for node in nodes {
        match node {
            TreeNode::Dir { children, .. } => {
                dirs += 1;
                let (child_dirs, child_files) = count_nodes(children);
                dirs += child_dirs;
                files += child_files;
            }
            TreeNode::File { .. } => files += 1,
        }
    }
    (dirs, files)
}

fn render_node(output: &mut String, node: &TreeNode) {
    match node {
        TreeNode::Dir {
            name,
            rel_path,
            children,
        } => {
            let href = format!("/tree/{}", encode_path_segments(rel_path));
            output.push_str(r#"<li class="tree-item"><div class="tree-row">"#);
            output.push_str(
                r#"<span class="tree-icon tree-icon--folder" aria-hidden="true">📁</span>"#,
            );
            output.push_str(r#"<span class="tree-name">"#);
            output.push_str(&escape_html(name));
            output.push_str("</span>");
            output.push_str(r#"<div class="tree-actions">"#);
            output.push_str(&format!(
                r#"<a class="action-link" href="{}">Open</a>"#,
                escape_attr(&href)
            ));
            output.push_str("</div></div>");
            if !children.is_empty() {
                output.push_str(r#"<ul>"#);
                for child in children {
                    render_node(output, child);
                }
                output.push_str("</ul>");
            }
            output.push_str("</li>");
        }
        TreeNode::File { name, url_path } => {
            output.push_str(r#"<li class="tree-item"><div class="tree-row">"#);
            output.push_str(
                r#"<span class="tree-icon tree-icon--file" aria-hidden="true">📄</span>"#,
            );
            output.push_str(r#"<span class="tree-name"><a href=""#);
            output.push_str(&escape_attr(url_path));
            output.push_str(r#"">"#);
            output.push_str(&escape_html(name));
            output.push_str("</a></span>");
            output.push_str(r#"<div class="tree-actions">"#);
            output.push_str(&format!(
                r#"<a class="action-link" href="{}">Open file</a>"#,
                escape_attr(url_path)
            ));
            output.push_str("</div></div></li>");
        }
    }
}

fn render_document(title: &str, body: &str, refresh_seconds: Option<u32>) -> String {
    let refresh_meta = refresh_seconds
        .map(|seconds| format!(r#"<meta http-equiv="refresh" content="{seconds}">"#))
        .unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  {refresh_meta}
  <link rel="stylesheet" href="{style_path}">
</head>
<body>
  {body}
</body>
</html>"#,
        title = escape_html(title),
        refresh_meta = refresh_meta,
        style_path = STYLE_PATH,
        body = body,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}
