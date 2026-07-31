use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{Paths, StateError, read_secure, write_atomic};

#[derive(Clone)]
pub struct Secret(String);
impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug, Default)]
pub struct CredentialStore {
    api: BTreeMap<String, Secret>,
    git: Vec<GitCredential>,
}

#[derive(Debug)]
struct GitCredential {
    server: String,
    path: String,
    token: Secret,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskStore {
    version: u8,
    #[serde(default)]
    api: BTreeMap<String, String>,
    #[serde(default)]
    git: Vec<DiskGit>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskGit {
    server: String,
    path: String,
    token: String,
}

impl CredentialStore {
    pub fn load(paths: &Paths) -> Result<Self, StateError> {
        let Some(bytes) = read_secure(&paths.credentials())? else {
            return Ok(Self::default());
        };
        let disk: DiskStore =
            serde_json::from_slice(&bytes).map_err(|e| StateError::Invalid(e.to_string()))?;
        if disk.version != 1 {
            return Err(StateError::Invalid(
                "unsupported credential store version".into(),
            ));
        }
        Ok(Self {
            api: disk
                .api
                .into_iter()
                .map(|(k, v)| (k, Secret::new(v)))
                .collect(),
            git: disk
                .git
                .into_iter()
                .map(|v| GitCredential {
                    server: v.server,
                    path: v.path,
                    token: Secret::new(v.token),
                })
                .collect(),
        })
    }
    pub fn save(&self, paths: &Paths) -> Result<(), StateError> {
        let disk = DiskStore {
            version: 1,
            api: self
                .api
                .iter()
                .map(|(k, v)| (k.clone(), v.expose().into()))
                .collect(),
            git: self
                .git
                .iter()
                .map(|v| DiskGit {
                    server: v.server.clone(),
                    path: v.path.clone(),
                    token: v.token.expose().into(),
                })
                .collect(),
        };
        let bytes = serde_json::to_vec(&disk).map_err(|e| StateError::Invalid(e.to_string()))?;
        write_atomic(&paths.credentials(), &bytes)
    }
    pub fn api_token(&self, server: &str) -> Option<&Secret> {
        self.api.get(server)
    }
    pub fn set_api_token(&mut self, server: String, token: String) {
        self.api.insert(server, Secret::new(token));
    }
    pub fn remove_api_token(&mut self, server: &str) -> bool {
        self.api.remove(server).is_some()
    }
    pub fn git_token(&self, server: &str, path: &str) -> Option<&Secret> {
        self.git
            .iter()
            .find(|v| v.server == server && v.path == path)
            .map(|v| &v.token)
    }
    pub fn set_git_token(&mut self, server: String, path: String, token: String) {
        self.git.retain(|v| v.server != server || v.path != path);
        self.git.push(GitCredential {
            server,
            path,
            token: Secret::new(token),
        });
    }
    pub fn remove_git_token(&mut self, server: &str, path: &str) -> bool {
        let before = self.git.len();
        self.git.retain(|v| v.server != server || v.path != path);
        before != self.git.len()
    }
}

pub fn plaintext_allowed(explicit: bool) -> bool {
    explicit || std::env::var("C6_ALLOW_PLAINTEXT_CREDENTIALS").as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_redacted_and_scoped_exactly() {
        let mut store = CredentialStore::default();
        store.set_api_token("work".into(), "c6c_v1_public_secret".into());
        store.set_git_token(
            "work".into(),
            "/git/a/b.git".into(),
            "c6g_v1_public_secret".into(),
        );
        let debug = format!("{store:?}");
        assert!(!debug.contains("public_secret"));
        assert!(store.git_token("work", "/git/a/b.git").is_some());
        assert!(store.git_token("work", "/git/a/other.git").is_none());
        assert!(store.git_token("other", "/git/a/b.git").is_none());
    }

    #[test]
    fn credential_store_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths {
            directory: temp.path().join("state"),
        };
        let mut store = CredentialStore::default();
        store.set_api_token("work".into(), "c6c_v1_x_y".into());
        store.set_git_token("work".into(), "/git/a/b.git".into(), "c6g_v1_x_y".into());
        store.save(&paths).unwrap();
        let loaded = CredentialStore::load(&paths).unwrap();
        assert_eq!(loaded.api_token("work").unwrap().expose(), "c6c_v1_x_y");
        assert_eq!(
            loaded.git_token("work", "/git/a/b.git").unwrap().expose(),
            "c6g_v1_x_y"
        );
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_credential_transactions_preserve_every_token() {
        use std::sync::{Arc, Barrier};
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
                let mut store = CredentialStore::load(&paths).unwrap();
                store.set_api_token(format!("server-{index}"), format!("c6c_v1_{index}_test"));
                store.save(&paths).unwrap();
            }));
        }
        for writer in writers {
            writer.join().unwrap();
        }
        let store = CredentialStore::load(&paths).unwrap();
        for index in 0..12 {
            assert_eq!(
                store
                    .api_token(&format!("server-{index}"))
                    .unwrap()
                    .expose(),
                format!("c6c_v1_{index}_test")
            );
        }
    }
}
