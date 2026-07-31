//! Safe, local management of C6-owned bare Git repositories.
//!
//! This crate intentionally does not implement a network transport. It invokes
//! `git` with discrete arguments and bounded stdin; no shell is involved.

use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use tempfile::NamedTempFile;
use thiserror::Error;

mod smart_http;

pub use smart_http::{SmartHttpBackend, SmartHttpError, SmartHttpLimits, SmartHttpResponse};

#[derive(Debug, Error)]
pub enum GitError {
    #[error("invalid repository slug: {0}")]
    InvalidSlug(String),
    #[error("invalid branch or revision: {0}")]
    InvalidRef(String),
    #[error("invalid repository path: {0}")]
    InvalidPath(String),
    #[error("invalid commit metadata: {0}")]
    InvalidMetadata(String),
    #[error("repository already exists: {0}")]
    AlreadyExists(String),
    #[error("repository not found: {0}")]
    NotFound(String),
    #[error("revision not found: {0}")]
    RevisionNotFound(String),
    #[error("operation exceeds configured limit: {0}")]
    LimitExceeded(String),
    #[error("repository changed concurrently")]
    ConcurrentUpdate,
    #[error("git executable was not found")]
    GitUnavailable,
    #[error("git command failed: {0}")]
    CommandFailed(String),
    #[error("unexpected git output: {0}")]
    InvalidOutput(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_changes: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub max_read_bytes: usize,
    pub max_log_entries: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_changes: 100,
            max_file_bytes: 1_048_576,
            max_total_bytes: 10_485_760,
            max_read_bytes: 2_097_152,
            max_log_entries: 500,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GitStore {
    root: PathBuf,
    limits: Limits,
}

impl GitStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, GitError> {
        Self::with_limits(root, Limits::default())
    }

    pub fn with_limits(root: impl AsRef<Path>, limits: Limits) -> Result<Self, GitError> {
        fs::create_dir_all(root.as_ref())?;
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(GitError::InvalidPath(
                "store root is not a directory".into(),
            ));
        }
        Ok(Self { root, limits })
    }

    pub fn create(&self, slug: &str) -> Result<Repository, GitError> {
        validate_slug(slug)?;
        let destination = self.root.join(format!("{slug}.git"));
        match fs::create_dir(&destination) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(GitError::AlreadyExists(slug.to_owned()));
            }
            Err(error) => return Err(error.into()),
        }
        let output = run_git(
            None,
            [
                OsStr::new("init"),
                OsStr::new("--bare"),
                OsStr::new("--initial-branch=main"),
                destination.as_os_str(),
            ],
            None,
            &[],
        )?;
        if !output.status.success() {
            let _ = fs::remove_dir_all(&destination);
            return Err(command_error(&output));
        }
        self.open(slug)
    }

    /// Imports an existing local repository. URL-like and non-canonical inputs
    /// are rejected; network cloning belongs in a separate trust boundary.
    pub fn import_local(
        &self,
        slug: &str,
        source: impl AsRef<Path>,
    ) -> Result<Repository, GitError> {
        validate_slug(slug)?;
        let source = source
            .as_ref()
            .canonicalize()
            .map_err(|_| GitError::InvalidPath("import source does not exist".into()))?;
        if !source.is_dir() {
            return Err(GitError::InvalidPath(
                "import source is not a directory".into(),
            ));
        }
        let destination = self.root.join(format!("{slug}.git"));
        ensure_absent(&destination, slug)?;
        let output = run_git(
            None,
            [
                OsStr::new("clone"),
                OsStr::new("--bare"),
                OsStr::new("--no-local"),
                OsStr::new("--"),
                source.as_os_str(),
                destination.as_os_str(),
            ],
            None,
            &[],
        )?;
        if !output.status.success() {
            let _ = fs::remove_dir_all(&destination);
            return Err(command_error(&output));
        }
        self.open(slug)
    }

    pub fn open(&self, slug: &str) -> Result<Repository, GitError> {
        validate_slug(slug)?;
        let expected = self.root.join(format!("{slug}.git"));
        let metadata =
            fs::symlink_metadata(&expected).map_err(|_| GitError::NotFound(slug.to_owned()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(GitError::InvalidPath(
                "repository must be a real directory".into(),
            ));
        }
        let canonical = expected.canonicalize()?;
        if canonical.parent() != Some(self.root.as_path()) {
            return Err(GitError::InvalidPath(
                "repository escaped store root".into(),
            ));
        }
        let probe = run_git(
            Some(&canonical),
            [OsStr::new("rev-parse"), OsStr::new("--is-bare-repository")],
            None,
            &[],
        )?;
        if !probe.status.success() || trim_ascii(&probe.stdout) != b"true" {
            return Err(GitError::InvalidPath("repository is not bare".into()));
        }
        let format = run_git(
            Some(&canonical),
            [OsStr::new("rev-parse"), OsStr::new("--show-object-format")],
            None,
            &[],
        )?;
        require_success(&format)?;
        let oid_len = match trim_ascii(&format.stdout) {
            b"sha1" => 40,
            b"sha256" => 64,
            _ => {
                return Err(GitError::InvalidOutput(
                    "unsupported Git object format".into(),
                ));
            }
        };
        Ok(Repository {
            path: canonical,
            limits: self.limits,
            oid_len,
        })
    }

    /// Permanently deletes one validated, store-owned bare repository.
    ///
    /// The repository is first atomically moved to a unique tombstone directory
    /// beneath the store root. This ensures requests can no longer open it before
    /// recursive removal begins and keeps recursive deletion scoped to a directory
    /// created by C6 itself.
    pub fn delete(&self, slug: &str) -> Result<(), GitError> {
        self.stage_delete(slug)?.commit()
    }

    /// Atomically removes a repository from the live namespace, but retains its
    /// contents until [`StagedDeletion::commit`] is called. Dropping or explicitly
    /// rolling back the guard restores the original repository when possible.
    ///
    /// This is intended for coordination with a SQL transaction: stage the Git
    /// repository, commit the database deletion, then commit this guard.
    pub fn stage_delete(&self, slug: &str) -> Result<StagedDeletion, GitError> {
        // `open` validates the slug, rejects symlinks/non-bare directories, and
        // proves the canonical repository is a direct child of this store.
        let repository = self.open(slug)?;
        let tombstone = tempfile::Builder::new()
            .prefix(".c6-delete-")
            .tempdir_in(&self.root)?
            .keep();
        let staged = tombstone.join("repository.git");
        if let Err(error) = fs::rename(&repository.path, &staged) {
            let _ = fs::remove_dir(&tombstone);
            return Err(error.into());
        }
        Ok(StagedDeletion {
            original: repository.path,
            tombstone,
            staged,
            completed: false,
        })
    }
}

/// A bare repository atomically moved out of the live store namespace.
///
/// The guard retains enough information to restore the repository if the
/// accompanying control-plane operation fails. If rollback itself fails, the
/// tombstone is deliberately left on disk rather than risking data loss.
#[derive(Debug)]
pub struct StagedDeletion {
    original: PathBuf,
    tombstone: PathBuf,
    staged: PathBuf,
    completed: bool,
}

impl StagedDeletion {
    /// Permanently removes the staged repository and its private tombstone.
    pub fn commit(mut self) -> Result<(), GitError> {
        fs::remove_dir_all(&self.tombstone)?;
        self.completed = true;
        Ok(())
    }

    /// Restores the repository to its original validated store path.
    pub fn rollback(mut self) -> Result<(), GitError> {
        self.restore()?;
        self.completed = true;
        Ok(())
    }

    fn restore(&self) -> Result<(), GitError> {
        if self.original.exists() {
            return Err(GitError::AlreadyExists(self.original.display().to_string()));
        }
        fs::rename(&self.staged, &self.original)?;
        fs::remove_dir(&self.tombstone)?;
        Ok(())
    }
}

impl Drop for StagedDeletion {
    fn drop(&mut self) {
        if !self.completed && self.staged.exists() && !self.original.exists() {
            // Best-effort safety net. A failure intentionally leaves the unique
            // tombstone untouched for operator recovery.
            if fs::rename(&self.staged, &self.original).is_ok() {
                let _ = fs::remove_dir(&self.tombstone);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Repository {
    path: PathBuf,
    limits: Limits,
    oid_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub oid: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub oid: String,
    pub parent_oids: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    pub oid: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Blob,
    Tree,
    Commit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitIdentity {
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileChange {
    Upsert { path: String, content: Vec<u8> },
    Delete { path: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum MergeResult {
    AlreadyUpToDate { oid: String },
    FastForward { oid: String },
    Merged { oid: String },
    Conflicted { paths: Vec<String> },
}

impl Repository {
    pub fn list_branches(&self) -> Result<Vec<Branch>, GitError> {
        let output = self.git(
            [
                OsStr::new("for-each-ref"),
                OsStr::new("--format=%(refname:strip=2)%00%(objectname)"),
                OsStr::new("refs/heads"),
            ],
            None,
            &[],
        )?;
        require_success(&output)?;
        let mut branches = Vec::new();
        for line in output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let fields: Vec<_> = line.splitn(2, |byte| *byte == 0).collect();
            if fields.len() != 2 {
                return Err(GitError::InvalidOutput("malformed branch record".into()));
            }
            branches.push(Branch {
                name: utf8(fields[0])?.to_owned(),
                oid: utf8(fields[1])?.to_owned(),
            });
        }
        Ok(branches)
    }

    pub fn list_commits(&self, revision: &str, limit: usize) -> Result<Vec<Commit>, GitError> {
        if limit == 0 || limit > self.limits.max_log_entries {
            return Err(GitError::LimitExceeded("invalid commit log limit".into()));
        }
        let oid = self.resolve_commit(revision)?;
        let limit_arg = format!("--max-count={limit}");
        let output = self.git(
            [
                OsStr::new("rev-list"),
                OsStr::new(&limit_arg),
                OsStr::new(&oid),
            ],
            None,
            &[],
        )?;
        require_success(&output)?;
        let mut commits = Vec::new();
        for oid in output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            commits.push(self.read_commit(utf8(oid)?)?);
        }
        Ok(commits)
    }

    pub fn list_tree(&self, revision: &str, recursive: bool) -> Result<Vec<TreeEntry>, GitError> {
        let oid = self.resolve_commit(revision)?;
        let mut args = vec![OsStr::new("ls-tree"), OsStr::new("-z"), OsStr::new("-l")];
        if recursive {
            args.push(OsStr::new("-r"));
        }
        args.push(OsStr::new(&oid));
        let output = self.git(args, None, &[])?;
        require_success(&output)?;
        parse_tree_entries(&output.stdout)
    }

    pub fn read_file(&self, revision: &str, path: &str) -> Result<Vec<u8>, GitError> {
        validate_repo_path(path)?;
        let oid = self.resolve_commit(revision)?;
        let output = self.git(
            [
                OsStr::new("ls-tree"),
                OsStr::new("-z"),
                OsStr::new("-l"),
                OsStr::new(&oid),
                OsStr::new("--"),
                OsStr::new(path),
            ],
            None,
            &[],
        )?;
        require_success(&output)?;
        let entry = parse_exact_tree_entry(&output.stdout, path)?;
        if entry.kind != EntryKind::Blob {
            return Err(GitError::InvalidPath("path is not a file".into()));
        }
        let size = entry
            .size
            .ok_or_else(|| GitError::InvalidOutput("blob has no size".into()))?;
        if size as usize > self.limits.max_read_bytes {
            return Err(GitError::LimitExceeded("file is too large to read".into()));
        }
        let output = self.git(
            [
                OsStr::new("cat-file"),
                OsStr::new("blob"),
                OsStr::new(&entry.oid),
            ],
            None,
            &[],
        )?;
        require_success(&output)?;
        Ok(output.stdout)
    }

    /// Creates or advances `branch` with a commit made from bounded file changes.
    /// When the branch does not exist, `start_revision` is required unless the
    /// repository is empty.
    pub fn commit_changes(
        &self,
        branch: &str,
        start_revision: Option<&str>,
        changes: &[FileChange],
        message: &str,
        identity: &CommitIdentity,
    ) -> Result<String, GitError> {
        validate_branch(branch)?;
        validate_identity(identity)?;
        validate_message(message)?;
        self.validate_changes(changes)?;

        let branch_ref = format!("refs/heads/{branch}");
        let current = self.resolve_ref_optional(&branch_ref)?;
        let parent = match (&current, start_revision) {
            (Some(oid), _) => Some(oid.clone()),
            (None, Some(revision)) => Some(self.resolve_commit(revision)?),
            (None, None) => None,
        };

        let index = NamedTempFile::new()?;
        let index_value = index.path().as_os_str();
        let index_env = [(OsStr::new("GIT_INDEX_FILE"), index_value)];
        // Git expects a nonexistent index, not an empty file.
        fs::remove_file(index.path())?;
        if let Some(parent) = &parent {
            let output = self.git(
                [OsStr::new("read-tree"), OsStr::new(parent)],
                None,
                &index_env,
            )?;
            require_success(&output)?;
        }

        for change in changes {
            match change {
                FileChange::Upsert { path, content } => {
                    let output = self.git(
                        [
                            OsStr::new("hash-object"),
                            OsStr::new("-w"),
                            OsStr::new("--stdin"),
                        ],
                        Some(content),
                        &[],
                    )?;
                    require_success(&output)?;
                    let blob_oid = utf8(trim_ascii(&output.stdout))?;
                    let cache = format!("100644,{blob_oid},{path}");
                    let output = self.git(
                        [
                            OsStr::new("update-index"),
                            OsStr::new("--add"),
                            OsStr::new("--cacheinfo"),
                            OsStr::new(&cache),
                        ],
                        None,
                        &index_env,
                    )?;
                    require_success(&output)?;
                }
                FileChange::Delete { path } => {
                    let removal = format!("0 {}\t{path}\n", "0".repeat(self.oid_len));
                    let output = self.git(
                        [OsStr::new("update-index"), OsStr::new("--index-info")],
                        Some(removal.as_bytes()),
                        &index_env,
                    )?;
                    require_success(&output)?;
                }
            }
        }
        let output = self.git([OsStr::new("write-tree")], None, &index_env)?;
        require_success(&output)?;
        let tree = utf8(trim_ascii(&output.stdout))?.to_owned();
        let mut args = vec![OsStr::new("commit-tree"), OsStr::new(&tree)];
        if let Some(parent) = &parent {
            args.push(OsStr::new("-p"));
            args.push(OsStr::new(parent));
        }
        let identity_env = identity_env(identity);
        let output = self.git(args, Some(message.as_bytes()), &identity_env)?;
        require_success(&output)?;
        let new_oid = utf8(trim_ascii(&output.stdout))?.to_owned();
        let zero_oid = "0".repeat(self.oid_len);
        let expected = current.as_deref().unwrap_or(&zero_oid);
        let output = self.git(
            [
                OsStr::new("update-ref"),
                OsStr::new(&branch_ref),
                OsStr::new(&new_oid),
                OsStr::new(expected),
            ],
            None,
            &[],
        )?;
        if !output.status.success() {
            if String::from_utf8_lossy(&output.stderr).contains("cannot lock ref") {
                return Err(GitError::ConcurrentUpdate);
            }
            return Err(command_error(&output));
        }
        Ok(new_oid)
    }

    pub fn merge(
        &self,
        target_branch: &str,
        source_revision: &str,
        message: &str,
        identity: &CommitIdentity,
    ) -> Result<MergeResult, GitError> {
        validate_branch(target_branch)?;
        validate_identity(identity)?;
        validate_message(message)?;
        let target_ref = format!("refs/heads/{target_branch}");
        let target = self
            .resolve_ref_optional(&target_ref)?
            .ok_or_else(|| GitError::RevisionNotFound(target_branch.to_owned()))?;
        let source = self.resolve_commit(source_revision)?;
        if self.is_ancestor(&source, &target)? {
            return Ok(MergeResult::AlreadyUpToDate { oid: target });
        }
        if self.is_ancestor(&target, &source)? {
            self.atomic_update(&target_ref, &source, &target)?;
            return Ok(MergeResult::FastForward { oid: source });
        }

        let output = self.git(
            [
                OsStr::new("merge-tree"),
                OsStr::new("--write-tree"),
                OsStr::new("--name-only"),
                OsStr::new("--no-messages"),
                OsStr::new("-z"),
                OsStr::new(&target),
                OsStr::new(&source),
            ],
            None,
            &[],
        )?;
        let fields: Vec<_> = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|field| !field.is_empty())
            .collect();
        if !output.status.success() {
            let paths = fields
                .iter()
                .skip(1)
                .map(|field| utf8(field).map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(MergeResult::Conflicted { paths });
        }
        let tree = fields
            .first()
            .ok_or_else(|| GitError::InvalidOutput("merge produced no tree".into()))
            .and_then(|field| utf8(field))?;
        let args = [
            OsStr::new("commit-tree"),
            OsStr::new(tree),
            OsStr::new("-p"),
            OsStr::new(&target),
            OsStr::new("-p"),
            OsStr::new(&source),
        ];
        let identity_env = identity_env(identity);
        let output = self.git(args, Some(message.as_bytes()), &identity_env)?;
        require_success(&output)?;
        let merged = utf8(trim_ascii(&output.stdout))?.to_owned();
        self.atomic_update(&target_ref, &merged, &target)?;
        Ok(MergeResult::Merged { oid: merged })
    }

    fn validate_changes(&self, changes: &[FileChange]) -> Result<(), GitError> {
        if changes.is_empty() || changes.len() > self.limits.max_changes {
            return Err(GitError::LimitExceeded(
                "invalid number of file changes".into(),
            ));
        }
        let mut total = 0usize;
        let mut paths = std::collections::HashSet::new();
        for change in changes {
            let (path, size) = match change {
                FileChange::Upsert { path, content } => (path, content.len()),
                FileChange::Delete { path } => (path, 0),
            };
            validate_repo_path(path)?;
            if !paths.insert(path) {
                return Err(GitError::InvalidPath("duplicate changed path".into()));
            }
            if size > self.limits.max_file_bytes {
                return Err(GitError::LimitExceeded("file change is too large".into()));
            }
            total = total
                .checked_add(size)
                .ok_or_else(|| GitError::LimitExceeded("change size overflow".into()))?;
        }
        if total > self.limits.max_total_bytes {
            return Err(GitError::LimitExceeded(
                "total change size is too large".into(),
            ));
        }
        Ok(())
    }

    fn read_commit(&self, oid: &str) -> Result<Commit, GitError> {
        let output = self.git(
            [
                OsStr::new("show"),
                OsStr::new("-s"),
                OsStr::new("--format=%H%x00%P%x00%an%x00%ae%x00%aI%x00%B"),
                OsStr::new(oid),
            ],
            None,
            &[],
        )?;
        require_success(&output)?;
        let fields: Vec<_> = output.stdout.splitn(6, |byte| *byte == 0).collect();
        if fields.len() != 6 {
            return Err(GitError::InvalidOutput("malformed commit record".into()));
        }
        Ok(Commit {
            oid: utf8(fields[0])?.to_owned(),
            parent_oids: utf8(fields[1])?
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            author_name: utf8(fields[2])?.to_owned(),
            author_email: utf8(fields[3])?.to_owned(),
            authored_at: utf8(fields[4])?.to_owned(),
            message: utf8(fields[5])?.trim_end_matches('\n').to_owned(),
        })
    }

    fn resolve_commit(&self, revision: &str) -> Result<String, GitError> {
        let spec = if is_oid(revision) {
            format!("{revision}^{{commit}}")
        } else {
            validate_branch(revision)?;
            format!("refs/heads/{revision}^{{commit}}")
        };
        let output = self.git(
            [
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new(&spec),
            ],
            None,
            &[],
        )?;
        if !output.status.success() {
            return Err(GitError::RevisionNotFound(revision.to_owned()));
        }
        Ok(utf8(trim_ascii(&output.stdout))?.to_owned())
    }

    fn resolve_ref_optional(&self, reference: &str) -> Result<Option<String>, GitError> {
        let output = self.git(
            [
                OsStr::new("rev-parse"),
                OsStr::new("--verify"),
                OsStr::new("--quiet"),
                OsStr::new(reference),
            ],
            None,
            &[],
        )?;
        if output.status.success() {
            Ok(Some(utf8(trim_ascii(&output.stdout))?.to_owned()))
        } else if output.status.code() == Some(1) {
            Ok(None)
        } else {
            Err(command_error(&output))
        }
    }

    fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool, GitError> {
        let output = self.git(
            [
                OsStr::new("merge-base"),
                OsStr::new("--is-ancestor"),
                OsStr::new(ancestor),
                OsStr::new(descendant),
            ],
            None,
            &[],
        )?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(command_error(&output)),
        }
    }

    fn atomic_update(&self, reference: &str, new: &str, old: &str) -> Result<(), GitError> {
        let output = self.git(
            [
                OsStr::new("update-ref"),
                OsStr::new(reference),
                OsStr::new(new),
                OsStr::new(old),
            ],
            None,
            &[],
        )?;
        if output.status.success() {
            Ok(())
        } else if String::from_utf8_lossy(&output.stderr).contains("cannot lock ref") {
            Err(GitError::ConcurrentUpdate)
        } else {
            Err(command_error(&output))
        }
    }

    fn git<I, S>(
        &self,
        args: I,
        stdin: Option<&[u8]>,
        env: &[(&OsStr, &OsStr)],
    ) -> Result<Output, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run_git(Some(&self.path), args, stdin, env)
    }
}

pub fn validate_slug(slug: &str) -> Result<(), GitError> {
    let valid = (1..=64).contains(&slug.len())
        && slug.as_bytes()[0].is_ascii_alphanumeric()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        && !slug.ends_with(".git");
    if valid {
        Ok(())
    } else {
        Err(GitError::InvalidSlug(slug.to_owned()))
    }
}

pub fn validate_branch(branch: &str) -> Result<(), GitError> {
    if branch.is_empty()
        || branch.len() > 255
        || branch.starts_with('-')
        || branch.starts_with('/')
        || branch.ends_with('/')
        || branch.ends_with('.')
        || branch.contains("..")
        || branch.contains("@{")
        || branch.contains("//")
        || branch.ends_with(".lock")
        || branch
            .split('/')
            .any(|component| component.starts_with('.'))
        || branch.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
        || branch
            .chars()
            .any(|ch| matches!(ch, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
    {
        return Err(GitError::InvalidRef(branch.to_owned()));
    }
    Ok(())
}

/// Validates a user-facing branch reference accepted by repository APIs.
pub fn validate_ref(reference: &str) -> Result<(), GitError> {
    validate_branch(reference)
}

pub fn validate_repo_path(path: &str) -> Result<(), GitError> {
    let invalid = path.is_empty()
        || path.len() > 4096
        || path.starts_with('/')
        || path.starts_with('-')
        || path.ends_with('/')
        || path.contains('\\')
        || path
            .bytes()
            .any(|byte| byte == 0 || byte < 0x20 || byte == 0x7f)
        || path.split('/').any(|part| {
            part.is_empty() || part == "." || part == ".." || part.eq_ignore_ascii_case(".git")
        });
    if invalid {
        Err(GitError::InvalidPath(path.to_owned()))
    } else {
        Ok(())
    }
}

fn validate_identity(identity: &CommitIdentity) -> Result<(), GitError> {
    let valid_name = !identity.name.trim().is_empty()
        && identity.name.len() <= 200
        && !identity
            .name
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r');
    let valid_email = !identity.email.is_empty()
        && identity.email.len() <= 320
        && identity.email.contains('@')
        && !identity
            .email
            .bytes()
            .any(|byte| byte <= b' ' || byte == 0x7f || byte == b'<' || byte == b'>');
    if valid_name && valid_email {
        Ok(())
    } else {
        Err(GitError::InvalidMetadata(
            "invalid author name or email".into(),
        ))
    }
}

fn validate_message(message: &str) -> Result<(), GitError> {
    if message.trim().is_empty() || message.len() > 65_536 || message.as_bytes().contains(&0) {
        Err(GitError::InvalidMetadata("invalid commit message".into()))
    } else {
        Ok(())
    }
}

fn identity_env(identity: &CommitIdentity) -> [(&OsStr, &OsStr); 4] {
    [
        (OsStr::new("GIT_AUTHOR_NAME"), OsStr::new(&identity.name)),
        (OsStr::new("GIT_AUTHOR_EMAIL"), OsStr::new(&identity.email)),
        (OsStr::new("GIT_COMMITTER_NAME"), OsStr::new(&identity.name)),
        (
            OsStr::new("GIT_COMMITTER_EMAIL"),
            OsStr::new(&identity.email),
        ),
    ]
}

fn ensure_absent(path: &Path, slug: &str) -> Result<(), GitError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(GitError::AlreadyExists(slug.to_owned())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn run_git<I, S>(
    git_dir: Option<&Path>,
    args: I,
    stdin: Option<&[u8]>,
    env: &[(&OsStr, &OsStr)],
) -> Result<Output, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    let path = std::env::var_os("PATH");
    command.env_clear();
    if let Some(path) = path {
        command.env("PATH", path);
    }
    if let Some(git_dir) = git_dir {
        command.arg("--git-dir").arg(git_dir);
    }
    command.args(args);
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            GitError::GitUnavailable
        } else {
            GitError::Io(error)
        }
    })?;
    if let Some(input) = stdin {
        child.stdin.take().expect("piped stdin").write_all(input)?;
    }
    Ok(child.wait_with_output()?)
}

fn parse_tree_entries(bytes: &[u8]) -> Result<Vec<TreeEntry>, GitError> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(parse_tree_record)
        .collect()
}

fn parse_exact_tree_entry(bytes: &[u8], expected_path: &str) -> Result<TreeEntry, GitError> {
    let entries = parse_tree_entries(bytes)?;
    match entries.as_slice() {
        [entry] if entry.path == expected_path => Ok(entry.clone()),
        [] => Err(GitError::NotFound(expected_path.to_owned())),
        _ => Err(GitError::InvalidOutput("ambiguous tree path".into())),
    }
}

fn parse_tree_record(record: &[u8]) -> Result<TreeEntry, GitError> {
    let tab = record
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or_else(|| GitError::InvalidOutput("tree record lacks path".into()))?;
    let header = utf8(&record[..tab])?;
    let path = utf8(&record[tab + 1..])?.to_owned();
    let fields: Vec<_> = header.split_whitespace().collect();
    if fields.len() != 4 {
        return Err(GitError::InvalidOutput("malformed tree record".into()));
    }
    let kind = match fields[1] {
        "blob" => EntryKind::Blob,
        "tree" => EntryKind::Tree,
        "commit" => EntryKind::Commit,
        _ => return Err(GitError::InvalidOutput("unknown tree entry type".into())),
    };
    let size = if fields[3] == "-" {
        None
    } else {
        Some(
            fields[3]
                .parse()
                .map_err(|_| GitError::InvalidOutput("invalid tree size".into()))?,
        )
    };
    Ok(TreeEntry {
        path,
        oid: fields[2].to_owned(),
        kind,
        size,
    })
}

fn is_oid(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn require_success(output: &Output) -> Result<(), GitError> {
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(output))
    }
}

fn command_error(output: &Output) -> GitError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    GitError::CommandFailed(stderr.trim().chars().take(500).collect())
}

fn utf8(bytes: &[u8]) -> Result<&str, GitError> {
    std::str::from_utf8(bytes)
        .map_err(|_| GitError::InvalidOutput("git emitted non-UTF-8 metadata".into()))
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, GitStore, Repository) {
        let temp = tempfile::tempdir().unwrap();
        let store = GitStore::new(temp.path().join("repos")).unwrap();
        let repo = store.create("demo").unwrap();
        (temp, store, repo)
    }

    fn identity() -> CommitIdentity {
        CommitIdentity {
            name: "C6 Test".into(),
            email: "test@c6.local".into(),
        }
    }

    fn upsert(path: &str, content: &str) -> FileChange {
        FileChange::Upsert {
            path: path.into(),
            content: content.as_bytes().to_vec(),
        }
    }

    fn commit(
        repo: &Repository,
        branch: &str,
        start: Option<&str>,
        changes: &[FileChange],
    ) -> String {
        repo.commit_changes(branch, start, changes, "test commit", &identity())
            .unwrap()
    }

    #[test]
    fn creates_bare_repository_and_rejects_collisions() {
        let (_temp, store, repo) = fixture();
        assert!(repo.list_branches().unwrap().is_empty());
        assert!(matches!(
            store.create("demo"),
            Err(GitError::AlreadyExists(_))
        ));
        assert!(store.open("demo").is_ok());
    }

    #[test]
    fn rejects_slug_traversal_and_symlink_repository() {
        let temp = tempfile::tempdir().unwrap();
        let store = GitStore::new(temp.path().join("repos")).unwrap();
        for slug in ["", ".", "..", "../escape", "a/b", "a.git", "a b", "💥"] {
            assert!(
                matches!(store.create(slug), Err(GitError::InvalidSlug(_))),
                "{slug}"
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temp.path(), temp.path().join("repos/linked.git")).unwrap();
            assert!(matches!(
                store.open("linked"),
                Err(GitError::InvalidPath(_))
            ));
        }
    }

    #[test]
    fn deletes_only_the_named_bare_repository_and_rejects_double_delete() {
        let (temp, store, repo) = fixture();
        commit(
            &repo,
            "main",
            None,
            &[upsert("private.txt", "confidential")],
        );
        let repository_path = temp.path().join("repos/demo.git");
        assert!(repository_path.join("objects").is_dir());

        store.delete("demo").unwrap();

        assert!(!repository_path.exists());
        assert!(temp.path().join("repos").is_dir());
        assert!(matches!(store.delete("demo"), Err(GitError::NotFound(_))));
        let tombstones = fs::read_dir(temp.path().join("repos"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".c6-delete-")
            })
            .count();
        assert_eq!(tombstones, 0);
    }

    #[test]
    fn deletion_rejects_traversal_non_bare_and_store_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repos");
        let store = GitStore::new(&root).unwrap();
        fs::write(root.join("root-marker"), "must survive").unwrap();
        fs::create_dir(root.join("ordinary.git")).unwrap();

        for slug in ["", ".", "..", "../repos", "a/b", "repos.git"] {
            assert!(
                matches!(store.delete(slug), Err(GitError::InvalidSlug(_))),
                "{slug:?}"
            );
        }
        assert!(matches!(
            store.delete("ordinary"),
            Err(GitError::InvalidPath(_))
        ));
        assert_eq!(
            fs::read_to_string(root.join("root-marker")).unwrap(),
            "must survive"
        );
        assert!(root.is_dir());
        assert!(temp.path().is_dir());
    }

    #[test]
    fn deletion_refuses_symlink_without_touching_its_target() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repos");
        let store = GitStore::new(&root).unwrap();
        let external = temp.path().join("external.git");
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .arg(&external)
                .status()
                .unwrap()
                .success()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&external, root.join("linked.git")).unwrap();
            assert!(matches!(
                store.delete("linked"),
                Err(GitError::InvalidPath(_))
            ));
            assert!(external.join("HEAD").is_file());
            assert!(root.join("linked.git").is_symlink());
        }
    }

    #[test]
    fn staged_deletion_rolls_back_explicitly_or_when_dropped() {
        let (_temp, store, repo) = fixture();
        commit(&repo, "main", None, &[upsert("data", "preserved")]);

        let staged = store.stage_delete("demo").unwrap();
        assert!(matches!(store.open("demo"), Err(GitError::NotFound(_))));
        staged.rollback().unwrap();
        assert_eq!(
            store
                .open("demo")
                .unwrap()
                .read_file("main", "data")
                .unwrap(),
            b"preserved"
        );

        {
            let _staged = store.stage_delete("demo").unwrap();
            assert!(matches!(store.open("demo"), Err(GitError::NotFound(_))));
        }
        assert_eq!(
            store
                .open("demo")
                .unwrap()
                .read_file("main", "data")
                .unwrap(),
            b"preserved"
        );
    }

    #[test]
    fn validates_branch_and_file_paths_against_injection_and_traversal() {
        for branch in ["main", "feature/ui", "agent/run-1", "UPPER_case"] {
            validate_branch(branch).unwrap();
        }
        for branch in [
            "",
            "-c",
            "../main",
            "a/.hidden",
            "a..b",
            "a.lock",
            "a b",
            "a~b",
            "a@{b",
            "a\\b",
            "a\nb",
        ] {
            assert!(validate_branch(branch).is_err(), "{branch:?}");
        }
        for path in ["README.md", "src/main.rs", "a b/file.txt"] {
            validate_repo_path(path).unwrap();
        }
        for path in [
            "",
            "/etc/passwd",
            "../x",
            "a/../x",
            "./x",
            "a//b",
            ".git/config",
            "a/.GIT/config",
            "-option",
            "a\\b",
            "a\tb",
        ] {
            assert!(validate_repo_path(path).is_err(), "{path:?}");
        }
    }

    #[test]
    fn commits_lists_and_reads_binary_files() {
        let (_temp, _store, repo) = fixture();
        let first = repo
            .commit_changes(
                "main",
                None,
                &[
                    upsert("README.md", "hello\n"),
                    FileChange::Upsert {
                        path: "assets/data.bin".into(),
                        content: vec![0, 1, 2, 255],
                    },
                ],
                "initial\n\nbody",
                &identity(),
            )
            .unwrap();
        assert_eq!(repo.read_file("main", "README.md").unwrap(), b"hello\n");
        assert_eq!(
            repo.read_file(&first, "assets/data.bin").unwrap(),
            vec![0, 1, 2, 255]
        );

        let branches = repo.list_branches().unwrap();
        assert_eq!(
            branches,
            vec![Branch {
                name: "main".into(),
                oid: first.clone()
            }]
        );
        let tree = repo.list_tree("main", true).unwrap();
        assert_eq!(
            tree.iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["README.md", "assets/data.bin"]
        );
        let log = repo.list_commits("main", 10).unwrap();
        assert_eq!(log[0].oid, first);
        assert_eq!(log[0].message, "initial\n\nbody");
        assert_eq!(log[0].author_email, "test@c6.local");
    }

    #[test]
    fn creates_branch_updates_and_deletes_without_worktree() {
        let (_temp, _store, repo) = fixture();
        let base = commit(
            &repo,
            "main",
            None,
            &[upsert("old.txt", "old"), upsert("keep.txt", "v1")],
        );
        let feature = commit(
            &repo,
            "feature/edit",
            Some(&base),
            &[
                FileChange::Delete {
                    path: "old.txt".into(),
                },
                upsert("keep.txt", "v2"),
            ],
        );
        assert!(matches!(
            repo.read_file("feature/edit", "old.txt"),
            Err(GitError::NotFound(_))
        ));
        assert_eq!(repo.read_file(&feature, "keep.txt").unwrap(), b"v2");
        assert_eq!(repo.read_file("main", "keep.txt").unwrap(), b"v1");
    }

    #[test]
    fn enforces_change_read_and_log_limits() {
        let temp = tempfile::tempdir().unwrap();
        let store = GitStore::with_limits(
            temp.path().join("repos"),
            Limits {
                max_changes: 1,
                max_file_bytes: 3,
                max_total_bytes: 3,
                max_read_bytes: 2,
                max_log_entries: 1,
            },
        )
        .unwrap();
        let repo = store.create("limits").unwrap();
        assert!(matches!(
            repo.commit_changes("main", None, &[upsert("a", "1234")], "m", &identity()),
            Err(GitError::LimitExceeded(_))
        ));
        commit(&repo, "main", None, &[upsert("a", "123")]);
        assert!(matches!(
            repo.read_file("main", "a"),
            Err(GitError::LimitExceeded(_))
        ));
        assert!(matches!(
            repo.list_commits("main", 2),
            Err(GitError::LimitExceeded(_))
        ));
        assert!(matches!(
            repo.commit_changes(
                "main",
                None,
                &[upsert("a", "1"), upsert("b", "2")],
                "m",
                &identity()
            ),
            Err(GitError::LimitExceeded(_))
        ));
    }

    #[test]
    fn rejects_duplicate_paths_bad_metadata_and_unknown_revisions() {
        let (_temp, _store, repo) = fixture();
        assert!(matches!(
            repo.commit_changes(
                "main",
                None,
                &[upsert("a", "1"), upsert("a", "2")],
                "m",
                &identity()
            ),
            Err(GitError::InvalidPath(_))
        ));
        let bad = CommitIdentity {
            name: "bad\nname".into(),
            email: "not-email".into(),
        };
        assert!(matches!(
            repo.commit_changes("main", None, &[upsert("a", "1")], "m", &bad),
            Err(GitError::InvalidMetadata(_))
        ));
        assert!(matches!(
            repo.list_commits("unknown", 1),
            Err(GitError::RevisionNotFound(_))
        ));
        assert!(matches!(
            repo.list_commits("--all", 1),
            Err(GitError::InvalidRef(_))
        ));
    }

    #[test]
    fn imports_local_repository_without_copying_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        assert!(
            Command::new("git")
                .args(["init", "-b", "main"])
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        fs::write(source.join("hello.txt"), "imported").unwrap();
        let status = Command::new("git")
            .current_dir(&source)
            .env("GIT_AUTHOR_NAME", "Import")
            .env("GIT_AUTHOR_EMAIL", "import@c6.local")
            .env("GIT_COMMITTER_NAME", "Import")
            .env("GIT_COMMITTER_EMAIL", "import@c6.local")
            .args(["add", "."])
            .status()
            .unwrap();
        assert!(status.success());
        assert!(
            Command::new("git")
                .current_dir(&source)
                .env("GIT_AUTHOR_NAME", "Import")
                .env("GIT_AUTHOR_EMAIL", "import@c6.local")
                .env("GIT_COMMITTER_NAME", "Import")
                .env("GIT_COMMITTER_EMAIL", "import@c6.local")
                .args(["commit", "-m", "initial"])
                .status()
                .unwrap()
                .success()
        );
        fs::write(source.join(".git/hooks/evil"), "not copied").unwrap();
        let store = GitStore::new(temp.path().join("managed")).unwrap();
        let imported = store.import_local("copy", &source).unwrap();
        assert_eq!(
            imported.read_file("main", "hello.txt").unwrap(),
            b"imported"
        );
        assert!(!temp.path().join("managed/copy.git/hooks/evil").exists());
    }

    #[test]
    fn merge_supports_fast_forward_clean_merge_and_already_up_to_date() {
        let (_temp, _store, repo) = fixture();
        let base = commit(&repo, "main", None, &[upsert("base", "1")]);
        let feature = commit(&repo, "feature", Some(&base), &[upsert("feature", "yes")]);
        assert_eq!(
            repo.merge("main", "feature", "ff", &identity()).unwrap(),
            MergeResult::FastForward {
                oid: feature.clone()
            }
        );
        assert_eq!(
            repo.merge("main", "feature", "noop", &identity()).unwrap(),
            MergeResult::AlreadyUpToDate {
                oid: feature.clone()
            }
        );

        let left = commit(&repo, "main", None, &[upsert("left", "yes")]);
        let right = commit(&repo, "topic", Some(&feature), &[upsert("right", "yes")]);
        let result = repo
            .merge("main", "topic", "merge topic", &identity())
            .unwrap();
        let merged = match result {
            MergeResult::Merged { oid } => oid,
            other => panic!("expected merge commit, got {other:?}"),
        };
        assert_ne!(merged, left);
        assert_eq!(repo.read_file("main", "left").unwrap(), b"yes");
        assert_eq!(repo.read_file("main", "right").unwrap(), b"yes");
        assert_eq!(
            repo.list_commits("main", 1).unwrap()[0].parent_oids,
            vec![left, right]
        );
    }

    #[test]
    fn merge_reports_conflicting_paths_without_moving_target() {
        let (_temp, _store, repo) = fixture();
        let base = commit(&repo, "main", None, &[upsert("same.txt", "base")]);
        let target = commit(&repo, "main", None, &[upsert("same.txt", "target")]);
        commit(&repo, "topic", Some(&base), &[upsert("same.txt", "source")]);
        let result = repo.merge("main", "topic", "merge", &identity()).unwrap();
        assert_eq!(
            result,
            MergeResult::Conflicted {
                paths: vec!["same.txt".into()]
            }
        );
        assert_eq!(
            repo.list_branches()
                .unwrap()
                .into_iter()
                .find(|branch| branch.name == "main")
                .unwrap()
                .oid,
            target
        );
        assert_eq!(repo.read_file("main", "same.txt").unwrap(), b"target");
    }
}
