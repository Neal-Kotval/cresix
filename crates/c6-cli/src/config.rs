use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("configuration directory is unavailable")]
    NoDirectory,
    #[error("unsafe local state at {path}: {reason}")]
    Unsafe { path: PathBuf, reason: String },
    #[error("I/O error for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("invalid local state: {0}")]
    Invalid(String),
    #[error("timed out waiting for another C6 process to finish updating local state")]
    LockTimeout,
    #[error("advisory state locking is unsupported on this platform")]
    LockUnsupported,
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub directory: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self, StateError> {
        if let Some(value) = env::var_os("C6_CONFIG_DIR") {
            if value.is_empty() {
                return Err(StateError::NoDirectory);
            }
            return Ok(Self {
                directory: PathBuf::from(value),
            });
        }
        Ok(Self {
            directory: dirs::config_dir()
                .ok_or(StateError::NoDirectory)?
                .join("c6"),
        })
    }
    pub fn config(&self) -> PathBuf {
        self.directory.join("config.toml")
    }
    pub fn credentials(&self) -> PathBuf {
        self.directory.join("credentials.json")
    }
    pub fn lock(&self) -> Result<StateLock, StateError> {
        StateLock::acquire(self)
    }
}

/// Kernel-held advisory lock shared by all C6 config and credential mutations.
/// The owner-only lock file may persist, but lock ownership cannot become stale.
pub struct StateLock {
    file: fs::File,
}

impl StateLock {
    fn acquire(paths: &Paths) -> Result<Self, StateError> {
        ensure_directory(&paths.directory)?;
        let path = paths.directory.join("state.lock");
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
        }
        let file = options.open(&path).map_err(|source| StateError::Io {
            path: path.clone(),
            source,
        })?;
        validate_metadata(
            &path,
            &file.metadata().map_err(|source| StateError::Io {
                path: path.clone(),
                source,
            })?,
        )?;
        #[cfg(unix)]
        {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let result = unsafe {
                    libc::flock(
                        std::os::fd::AsRawFd::as_raw_fd(&file),
                        libc::LOCK_EX | libc::LOCK_NB,
                    )
                };
                if result == 0 {
                    break;
                }
                let error = io::Error::last_os_error();
                let code = error.raw_os_error();
                if code != Some(libc::EWOULDBLOCK) && code != Some(libc::EAGAIN) {
                    return Err(StateError::Io {
                        path,
                        source: error,
                    });
                }
                if Instant::now() >= deadline {
                    return Err(StateError::LockTimeout);
                }
                thread::sleep(Duration::from_millis(25));
            }
            Ok(Self { file })
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            Err(StateError::LockUnsupported)
        }
    }
}

impl Drop for StateLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::flock(std::os::fd::AsRawFd::as_raw_fd(&self.file), libc::LOCK_UN);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u8,
    pub default_server: Option<String>,
    #[serde(default)]
    pub plaintext_credentials: bool,
    pub servers: BTreeMap<String, Server>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub base_url: String,
    pub server_id: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_http_localhost: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            default_server: None,
            plaintext_credentials: false,
            servers: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self, StateError> {
        match read_secure(&paths.config())? {
            None => Ok(Self::default()),
            Some(bytes) => {
                let value: Self =
                    toml::from_slice(&bytes).map_err(|e| StateError::Invalid(e.to_string()))?;
                if value.version != 1 {
                    return Err(StateError::Invalid("unsupported config version".into()));
                }
                Ok(value)
            }
        }
    }
    pub fn save(&self, paths: &Paths) -> Result<(), StateError> {
        let bytes = toml::to_string_pretty(self).map_err(|e| StateError::Invalid(e.to_string()))?;
        write_atomic(&paths.config(), bytes.as_bytes())
    }
}

pub(crate) fn read_secure(path: &Path) -> Result<Option<Vec<u8>>, StateError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(StateError::Io {
                path: path.into(),
                source,
            });
        }
    };
    validate_metadata(path, &metadata)?;
    fs::read(path).map(Some).map_err(|source| StateError::Io {
        path: path.into(),
        source,
    })
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StateError> {
    let directory = path.parent().ok_or(StateError::NoDirectory)?;
    ensure_directory(directory)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        validate_metadata(path, &metadata)?;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| StateError::Invalid("invalid state filename".into()))?;
    let temporary = directory.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|source| StateError::Io {
            path: temporary.clone(),
            source,
        })?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|source| StateError::Io {
                path: temporary.clone(),
                source,
            })?;
        fs::rename(&temporary, path).map_err(|source| StateError::Io {
            path: path.into(),
            source,
        })?;
        let dir = fs::File::open(directory).map_err(|source| StateError::Io {
            path: directory.into(),
            source,
        })?;
        dir.sync_all().map_err(|source| StateError::Io {
            path: directory.into(),
            source,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn ensure_directory(path: &Path) -> Result<(), StateError> {
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path).map_err(|source| StateError::Io {
            path: path.into(),
            source,
        })?;
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| StateError::Io {
        path: path.into(),
        source,
    })?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(StateError::Unsafe {
            path: path.into(),
            reason: "must be a real directory".into(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(StateError::Unsafe {
                path: path.into(),
                reason: "must be owned by the current user with mode 0700 or stricter".into(),
            });
        }
    }
    Ok(())
}

fn validate_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), StateError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(StateError::Unsafe {
            path: path.into(),
            reason: "must be a regular file, not a symlink".into(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(StateError::Unsafe {
                path: path.into(),
                reason: "must be owned by the current user with mode 0600 or stricter".into(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip_is_owner_only() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: temp.path().join("state"),
        };
        let mut config = Config {
            default_server: Some("work".into()),
            ..Config::default()
        };
        config.servers.insert(
            "work".into(),
            Server {
                base_url: "https://c6.example".into(),
                server_id: "server-1".into(),
                allow_http_localhost: false,
            },
        );
        config.save(&paths).unwrap();
        assert_eq!(Config::load(&paths).unwrap(), config);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(paths.config()).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&paths.directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_and_broad_mode() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: temp.path().join("state"),
        };
        fs::create_dir(&paths.directory).unwrap();
        fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
        let target = temp.path().join("target");
        fs::write(&target, "version=1").unwrap();
        symlink(&target, paths.config()).unwrap();
        assert!(matches!(
            Config::load(&paths),
            Err(StateError::Unsafe { .. })
        ));
        fs::remove_file(paths.config()).unwrap();
        fs::write(paths.config(), "version=1\nservers={}").unwrap();
        fs::set_permissions(paths.config(), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            Config::load(&paths),
            Err(StateError::Unsafe { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_config_transactions_preserve_every_writer() {
        use std::{
            os::unix::fs::PermissionsExt,
            sync::{Arc, Barrier},
        };
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: temp.path().join("state"),
        };
        let barrier = Arc::new(Barrier::new(12));
        let mut writers = Vec::new();
        for index in 0..12 {
            let paths = paths.clone();
            let barrier = barrier.clone();
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                let _lock = paths.lock().unwrap();
                let mut config = Config::load(&paths).unwrap();
                config.servers.insert(
                    format!("server-{index}"),
                    Server {
                        base_url: format!("https://server-{index}.example"),
                        server_id: format!("id-{index}"),
                        allow_http_localhost: false,
                    },
                );
                config.save(&paths).unwrap();
            }));
        }
        for writer in writers {
            writer.join().unwrap();
        }
        let config = Config::load(&paths).unwrap();
        assert_eq!(config.servers.len(), 12);
        assert_eq!(
            fs::metadata(paths.config()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(paths.directory.join("state.lock"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let first = paths.lock().unwrap();
        drop(first);
        assert!(
            paths.lock().is_ok(),
            "kernel lock remained stale after drop"
        );
    }

    #[cfg(unix)]
    #[test]
    fn lock_rejects_symlinks_and_broad_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: temp.path().join("state"),
        };
        fs::create_dir(&paths.directory).unwrap();
        fs::set_permissions(&paths.directory, fs::Permissions::from_mode(0o700)).unwrap();
        let target = temp.path().join("target-lock");
        fs::write(&target, "").unwrap();
        let lock_path = paths.directory.join("state.lock");
        symlink(&target, &lock_path).unwrap();
        assert!(paths.lock().is_err(), "accepted a symlink lock file");
        fs::remove_file(&lock_path).unwrap();
        fs::write(&lock_path, "").unwrap();
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(paths.lock(), Err(StateError::Unsafe { .. })));
    }
}
