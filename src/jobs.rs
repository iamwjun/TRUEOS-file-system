use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::{RwLock, mpsc};
use tokio::time::sleep;

const JOB_CHANNEL_CAPACITY: usize = 64;
pub const DOWNLOAD_STAGING_DIR: &str = ".job-downloads";

/// Public request model for file-system jobs.
///
/// New job types can be added as enum variants while keeping the same
/// `JobQueue::enqueue` entry point stable for Rust callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRequest {
    Move {
        source: String,
        destination: String,
    },
    Delete {
        target: String,
    },
    Upload {
        directory: String,
        filename: String,
        bytes: Vec<u8>,
    },
    Download {
        source: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobError {
    message: String,
}

#[derive(Clone)]
pub struct JobQueue {
    sender: mpsc::Sender<QueuedJob>,
    store: Arc<JobStore>,
    next_id: Arc<AtomicU64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSnapshot {
    pub id: u64,
    pub kind: JobKind,
    pub status: JobStatus,
    pub summary: String,
    pub detail: String,
    pub result_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Move,
    Delete,
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

struct JobStore {
    jobs: RwLock<BTreeMap<u64, JobSnapshot>>,
}

struct QueuedJob {
    id: u64,
    request: JobRequest,
}

struct JobOutcome {
    result_path: Option<String>,
}

impl JobRequest {
    pub fn move_path(
        source: impl AsRef<str>,
        destination: impl AsRef<str>,
    ) -> Result<Self, JobError> {
        let source = normalize_required_path(source.as_ref(), "source path")?;
        let destination = normalize_required_path(destination.as_ref(), "destination path")?;
        Ok(Self::Move {
            source,
            destination,
        })
    }

    pub fn delete(target: impl AsRef<str>) -> Result<Self, JobError> {
        let target = normalize_required_path(target.as_ref(), "target path")?;
        Ok(Self::Delete { target })
    }

    pub fn upload(
        directory: impl AsRef<str>,
        filename: impl AsRef<str>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, JobError> {
        let directory = normalize_optional_path(directory.as_ref(), "target directory")?;
        let filename = normalize_filename(filename.as_ref())?;
        Ok(Self::Upload {
            directory,
            filename,
            bytes: bytes.into(),
        })
    }

    pub fn download(source: impl AsRef<str>) -> Result<Self, JobError> {
        let source = normalize_required_path(source.as_ref(), "source path")?;
        Ok(Self::Download { source })
    }

    fn kind(&self) -> JobKind {
        match self {
            Self::Move { .. } => JobKind::Move,
            Self::Delete { .. } => JobKind::Delete,
            Self::Upload { .. } => JobKind::Upload,
            Self::Download { .. } => JobKind::Download,
        }
    }

    fn summary(&self) -> &'static str {
        match self {
            Self::Move { .. } => "Move file or directory",
            Self::Delete { .. } => "Delete file or directory",
            Self::Upload { .. } => "Upload file",
            Self::Download { .. } => "Prepare download artifact",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Move {
                source,
                destination,
            } => format!("{source} -> {destination}"),
            Self::Delete { target } => target.clone(),
            Self::Upload {
                directory,
                filename,
                ..
            } => join_rel_path(directory, filename),
            Self::Download { source } => source.clone(),
        }
    }
}

impl JobError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for JobError {}

impl JobQueue {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let (sender, receiver) = mpsc::channel(JOB_CHANNEL_CAPACITY);
        let store = Arc::new(JobStore::default());
        let worker_store = store.clone();

        tokio::spawn(async move {
            run_worker(root, worker_store, receiver).await;
        });

        Self {
            sender,
            store,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub async fn enqueue(&self, request: JobRequest) -> Result<JobSnapshot, JobError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let snapshot = JobSnapshot {
            id,
            kind: request.kind(),
            status: JobStatus::Queued,
            summary: request.summary().to_string(),
            detail: request.detail(),
            result_path: None,
            error: None,
        };

        self.store.insert(snapshot.clone()).await;

        if self.sender.send(QueuedJob { id, request }).await.is_err() {
            let error = JobError::new("job queue is unavailable");
            self.store.mark_failed(id, error.to_string()).await;
            return Err(error);
        }

        Ok(snapshot)
    }

    pub async fn enqueue_move(
        &self,
        source: impl AsRef<str>,
        destination: impl AsRef<str>,
    ) -> Result<JobSnapshot, JobError> {
        self.enqueue(JobRequest::move_path(source, destination)?)
            .await
    }

    pub async fn enqueue_delete(&self, target: impl AsRef<str>) -> Result<JobSnapshot, JobError> {
        self.enqueue(JobRequest::delete(target)?).await
    }

    pub async fn enqueue_upload(
        &self,
        directory: impl AsRef<str>,
        filename: impl AsRef<str>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<JobSnapshot, JobError> {
        self.enqueue(JobRequest::upload(directory, filename, bytes)?)
            .await
    }

    pub async fn enqueue_download(&self, source: impl AsRef<str>) -> Result<JobSnapshot, JobError> {
        self.enqueue(JobRequest::download(source)?).await
    }

    pub async fn list(&self, limit: usize) -> Vec<JobSnapshot> {
        self.store.list(limit).await
    }

    pub async fn get(&self, id: u64) -> Option<JobSnapshot> {
        self.store.get(id).await
    }

    pub async fn wait_for_terminal(&self, id: u64, poll_interval: Duration) -> Option<JobSnapshot> {
        let interval = if poll_interval.is_zero() {
            Duration::from_millis(10)
        } else {
            poll_interval
        };

        loop {
            let snapshot = self.get(id).await?;
            if snapshot.status.is_terminal() {
                return Some(snapshot);
            }
            sleep(interval).await;
        }
    }
}

impl JobSnapshot {
    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    pub fn status_class(&self) -> &'static str {
        self.status.css_class()
    }
}

impl JobKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Delete => "Delete",
            Self::Upload => "Upload",
            Self::Download => "Download",
        }
    }
}

impl JobStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

impl Default for JobStore {
    fn default() -> Self {
        Self {
            jobs: RwLock::new(BTreeMap::new()),
        }
    }
}

impl JobStore {
    async fn insert(&self, snapshot: JobSnapshot) {
        self.jobs.write().await.insert(snapshot.id, snapshot);
    }

    async fn get(&self, id: u64) -> Option<JobSnapshot> {
        self.jobs.read().await.get(&id).cloned()
    }

    async fn list(&self, limit: usize) -> Vec<JobSnapshot> {
        self.jobs
            .read()
            .await
            .iter()
            .rev()
            .take(limit)
            .map(|(_, job)| job.clone())
            .collect()
    }

    async fn mark_running(&self, id: u64) {
        if let Some(job) = self.jobs.write().await.get_mut(&id) {
            job.status = JobStatus::Running;
            job.error = None;
        }
    }

    async fn mark_succeeded(&self, id: u64, result_path: Option<String>) {
        if let Some(job) = self.jobs.write().await.get_mut(&id) {
            job.status = JobStatus::Succeeded;
            job.result_path = result_path;
            job.error = None;
        }
    }

    async fn mark_failed(&self, id: u64, error: String) {
        if let Some(job) = self.jobs.write().await.get_mut(&id) {
            job.status = JobStatus::Failed;
            job.error = Some(error);
        }
    }
}

async fn run_worker(root: PathBuf, store: Arc<JobStore>, mut receiver: mpsc::Receiver<QueuedJob>) {
    while let Some(job) = receiver.recv().await {
        store.mark_running(job.id).await;

        match execute_job(&root, job.id, job.request).await {
            Ok(outcome) => store.mark_succeeded(job.id, outcome.result_path).await,
            Err(error) => store.mark_failed(job.id, error.to_string()).await,
        }
    }
}

async fn execute_job(
    root: &Path,
    job_id: u64,
    request: JobRequest,
) -> Result<JobOutcome, JobError> {
    match request {
        JobRequest::Move {
            source,
            destination,
        } => {
            let source_path = resolve_existing_path(root, &source).await?;
            let destination_path = resolve_new_path(root, &destination)?;
            create_parent_dirs(&destination_path).await?;
            tokio::fs::rename(&source_path, &destination_path)
                .await
                .map_err(|error| JobError::new(format!("move failed: {error}")))?;

            Ok(JobOutcome {
                result_path: Some(serve_path_for_relative(&destination)),
            })
        }
        JobRequest::Delete { target } => {
            let target_path = resolve_existing_path(root, &target).await?;
            let metadata = tokio::fs::metadata(&target_path)
                .await
                .map_err(|error| JobError::new(format!("delete failed: {error}")))?;
            if metadata.is_dir() {
                tokio::fs::remove_dir_all(&target_path)
                    .await
                    .map_err(|error| JobError::new(format!("delete failed: {error}")))?;
            } else {
                tokio::fs::remove_file(&target_path)
                    .await
                    .map_err(|error| JobError::new(format!("delete failed: {error}")))?;
            }

            Ok(JobOutcome { result_path: None })
        }
        JobRequest::Upload {
            directory,
            filename,
            bytes,
        } => {
            let relative = join_rel_path(&directory, &filename);
            let target_path = resolve_new_path(root, &relative)?;
            create_parent_dirs(&target_path).await?;
            tokio::fs::write(&target_path, bytes)
                .await
                .map_err(|error| JobError::new(format!("upload failed: {error}")))?;

            Ok(JobOutcome {
                result_path: Some(serve_path_for_relative(&relative)),
            })
        }
        JobRequest::Download { source } => {
            let source_path = resolve_existing_path(root, &source).await?;
            let metadata = tokio::fs::metadata(&source_path)
                .await
                .map_err(|error| JobError::new(format!("download preparation failed: {error}")))?;
            if !metadata.is_file() {
                return Err(JobError::new("download preparation only supports files"));
            }

            let filename = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| JobError::new("download source must include a valid filename"))?;
            let staged_relative = format!("{DOWNLOAD_STAGING_DIR}/job-{job_id}/{filename}");
            let staged_path = resolve_new_path(root, &staged_relative)?;
            create_parent_dirs(&staged_path).await?;
            tokio::fs::copy(&source_path, &staged_path)
                .await
                .map_err(|error| JobError::new(format!("download preparation failed: {error}")))?;

            Ok(JobOutcome {
                result_path: Some(serve_path_for_relative(&staged_relative)),
            })
        }
    }
}

async fn resolve_existing_path(root: &Path, relative: &str) -> Result<PathBuf, JobError> {
    let path = resolve_new_path(root, relative)?;
    let exists = tokio::fs::try_exists(&path)
        .await
        .map_err(|error| JobError::new(format!("failed to check path existence: {error}")))?;
    if !exists {
        return Err(JobError::new(format!("path does not exist: {relative}")));
    }
    Ok(path)
}

fn resolve_new_path(root: &Path, relative: &str) -> Result<PathBuf, JobError> {
    let relative = normalize_required_path(relative, "path")?;
    Ok(root.join(relative))
}

fn normalize_required_path(value: &str, label: &str) -> Result<String, JobError> {
    let normalized = normalize_path_value(value)?;
    if normalized.is_empty() {
        return Err(JobError::new(format!("{label} cannot be empty")));
    }
    Ok(normalized)
}

fn normalize_optional_path(value: &str, label: &str) -> Result<String, JobError> {
    normalize_path_value(value).map_err(|error| JobError::new(format!("{label} {error}")))
}

fn normalize_path_value(value: &str) -> Result<String, JobError> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.contains('\\') {
        return Err(JobError::new(
            "must use forward-slash relative paths inside the configured root",
        ));
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "..")
    {
        return Err(JobError::new("must stay inside the configured root"));
    }
    Ok(trimmed.to_string())
}

fn normalize_filename(value: &str) -> Result<String, JobError> {
    let filename = value.trim();
    if filename.is_empty() {
        return Err(JobError::new("uploaded file must include a filename"));
    }
    if filename == "." || filename == ".." || filename.contains('/') || filename.contains('\\') {
        return Err(JobError::new(
            "uploaded filename must be a single path segment",
        ));
    }
    Ok(filename.to_string())
}

fn join_rel_path(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        suffix.to_string()
    } else {
        format!("{prefix}/{suffix}")
    }
}

async fn create_parent_dirs(path: &Path) -> Result<(), JobError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            JobError::new(format!("failed to prepare parent directories: {error}"))
        })?;
    }
    Ok(())
}

fn serve_path_for_relative(relative: &str) -> String {
    format!("/{}", encode_path_segments(relative))
}

fn encode_path_segments(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_segment(segment: &str) -> String {
    let mut output = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char)
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}
