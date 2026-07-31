use std::{
    fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use c6_cloud_core::{SecretToken, TokenClass};
use serde::Deserialize;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_CREDENTIAL_BYTES: u64 = 4 * 1024;
const MIN_CREDENTIAL_BYTES: usize = 32;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ConnectorConfig {
    pub cloud_origin: String,
    pub local_origin: String,
    pub installation_id: Uuid,
    pub binding_id: Uuid,
    pub local_workspace_id: Uuid,
    pub cloud_credential_file: PathBuf,
    pub local_credential_file: PathBuf,
    #[serde(default)]
    pub allow_insecure_cloud_loopback: bool,
    #[serde(default = "default_catalog_interval")]
    pub catalog_interval_seconds: u64,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_seconds: u64,
    #[serde(default = "default_max_in_flight")]
    pub max_in_flight: usize,
}

impl fmt::Debug for ConnectorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorConfig")
            .field("cloud_origin", &self.cloud_origin)
            .field("local_origin", &self.local_origin)
            .field("installation_id", &self.installation_id)
            .field("cloud_credential_file", &self.cloud_credential_file)
            .field("local_credential_file", &self.local_credential_file)
            .field(
                "allow_insecure_cloud_loopback",
                &self.allow_insecure_cloud_loopback,
            )
            .field("catalog_interval_seconds", &self.catalog_interval_seconds)
            .field("request_timeout_seconds", &self.request_timeout_seconds)
            .field("max_in_flight", &self.max_in_flight)
            .finish()
    }
}

const fn default_catalog_interval() -> u64 {
    60
}
const fn default_request_timeout() -> u64 {
    30
}
const fn default_max_in_flight() -> usize {
    32
}

#[derive(Clone)]
pub struct Credentials {
    cloud: SecretToken,
    local: String,
}

impl Credentials {
    pub fn cloud(&self) -> &SecretToken {
        &self.cloud
    }
    pub fn local(&self) -> &str {
        &self.local
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credentials([REDACTED])")
    }
}

#[derive(Clone, Debug)]
pub struct LoadedConfig {
    pub config: ConnectorConfig,
    pub cloud_origin: Url,
    pub local_origin: Url,
    pub credentials: Credentials,
}

impl LoadedConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        ensure_owner_only_regular_file(path)?;
        let raw = read_bounded(path, MAX_CONFIG_BYTES)?;
        let config: ConnectorConfig =
            toml::from_str(&raw).map_err(|_| ConfigError::InvalidConfig)?;
        validate_limits(&config)?;
        let cloud_origin =
            validate_cloud_origin(&config.cloud_origin, config.allow_insecure_cloud_loopback)?;
        let local_origin = validate_local_origin(&config.local_origin)?;
        let cloud = SecretToken::parse(load_credential(&config.cloud_credential_file)?)
            .map_err(|_| ConfigError::InvalidCredential)?;
        if cloud.parsed().class != TokenClass::Connector {
            return Err(ConfigError::InvalidCredential);
        }
        let local = load_credential(&config.local_credential_file)?;
        if same_file(path, &config.cloud_credential_file)
            || same_file(path, &config.local_credential_file)
        {
            return Err(ConfigError::CredentialMustBeSeparate);
        }
        Ok(Self {
            config,
            cloud_origin,
            local_origin,
            credentials: Credentials { cloud, local },
        })
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.config.request_timeout_seconds)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("configuration or credential path is not a regular file")]
    NotRegularFile,
    #[error("configuration and credential files must be owner-only (mode 0600)")]
    UnsafePermissions,
    #[error("could not inspect or read a configuration file")]
    Io,
    #[error("configuration file is too large or is not UTF-8")]
    InvalidConfig,
    #[error("credential file is invalid")]
    InvalidCredential,
    #[error("credentials must be stored separately from connector configuration")]
    CredentialMustBeSeparate,
    #[error("cloud_origin must be an HTTPS origin; loopback HTTP requires explicit opt-in")]
    InvalidCloudOrigin,
    #[error("local_origin must be a literal http://127.0.0.1:<port> origin")]
    InvalidLocalOrigin,
    #[error("connector limits are outside their safe ranges")]
    InvalidLimits,
}

fn validate_limits(config: &ConnectorConfig) -> Result<(), ConfigError> {
    if !(10..=86_400).contains(&config.catalog_interval_seconds)
        || !(1..=120).contains(&config.request_timeout_seconds)
        || !(1..=32).contains(&config.max_in_flight)
    {
        return Err(ConfigError::InvalidLimits);
    }
    Ok(())
}

pub fn validate_cloud_origin(input: &str, allow_loopback_http: bool) -> Result<Url, ConfigError> {
    let url = parse_bare_origin(input).map_err(|_| ConfigError::InvalidCloudOrigin)?;
    let valid = url.scheme() == "https"
        || (allow_loopback_http
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "::1")));
    valid.then_some(url).ok_or(ConfigError::InvalidCloudOrigin)
}

pub fn validate_local_origin(input: &str) -> Result<Url, ConfigError> {
    let url = parse_bare_origin(input).map_err(|_| ConfigError::InvalidLocalOrigin)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || url.port() == Some(0)
    {
        return Err(ConfigError::InvalidLocalOrigin);
    }
    Ok(url)
}

fn parse_bare_origin(input: &str) -> Result<Url, ()> {
    let url = Url::parse(input).map_err(|_| ())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.host_str().is_none()
    {
        return Err(());
    }
    Ok(url)
}

fn load_credential(path: &Path) -> Result<String, ConfigError> {
    ensure_owner_only_regular_file(path)?;
    let value =
        read_bounded(path, MAX_CREDENTIAL_BYTES).map_err(|_| ConfigError::InvalidCredential)?;
    let value = value.strip_suffix('\n').unwrap_or(&value);
    let value = value.strip_suffix('\r').unwrap_or(value);
    if value.len() < MIN_CREDENTIAL_BYTES
        || value.len() > MAX_CREDENTIAL_BYTES as usize
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'"' && byte != b'\\')
    {
        return Err(ConfigError::InvalidCredential);
    }
    Ok(value.to_owned())
}

fn read_bounded(path: &Path, max: u64) -> Result<String, ConfigError> {
    let metadata = fs::metadata(path).map_err(|_| ConfigError::Io)?;
    if metadata.len() > max {
        return Err(ConfigError::InvalidConfig);
    }
    fs::read_to_string(path).map_err(|_| ConfigError::Io)
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(unix)]
fn ensure_owner_only_regular_file(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let metadata = fs::symlink_metadata(path).map_err(|_| ConfigError::Io)?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 {
        return Err(ConfigError::NotRegularFile);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(ConfigError::UnsafePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_owner_only_regular_file(path: &Path) -> Result<(), ConfigError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ConfigError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(ConfigError::NotRegularFile);
    }
    // The initial connector deliberately fails closed on platforms where this
    // build cannot verify an owner-only ACL.
    Err(ConfigError::UnsafePermissions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    fn private_file(path: &Path, value: &str) {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(path).unwrap();
        use std::io::Write;
        file.write_all(value.as_bytes()).unwrap();
    }

    #[test]
    fn origins_are_pinned_to_safe_authorities() {
        assert!(validate_local_origin("http://127.0.0.1:8787").is_ok());
        for bad in [
            "http://localhost:8787",
            "http://127.0.0.1",
            "http://127.0.0.1:8787/path",
            "http://127.0.0.1:8787?target=x",
            "http://user@127.0.0.1:8787",
            "https://127.0.0.1:8787",
            "http://[::1]:8787",
        ] {
            assert_eq!(
                validate_local_origin(bad),
                Err(ConfigError::InvalidLocalOrigin)
            );
        }
        assert!(validate_cloud_origin("https://cresix.example", false).is_ok());
        assert!(validate_cloud_origin("http://127.0.0.1:9797", true).is_ok());
        assert!(validate_cloud_origin("http://127.0.0.1:9797", false).is_err());
        assert!(validate_cloud_origin("http://localhost:9797", true).is_err());
        assert!(validate_cloud_origin("https://example.com/a", false).is_err());
    }

    #[test]
    fn debug_never_prints_credentials() {
        let credentials = Credentials {
            cloud: SecretToken::parse("c6x_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")
                .unwrap(),
            local: "local-super-secret".into(),
        };
        let printed = format!("{credentials:?}");
        assert_eq!(printed, "Credentials([REDACTED])");
        assert!(!printed.contains("secret"));
    }

    #[test]
    fn loads_separate_private_files_and_rejects_unknown_config() {
        let dir = tempfile::tempdir().unwrap();
        let cloud = dir.path().join("cloud-token");
        let local = dir.path().join("local-token");
        let config = dir.path().join("connector.toml");
        private_file(
            &cloud,
            "c6x_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB\n",
        );
        private_file(&local, "llllllllllllllllllllllllllllllll\n");
        private_file(
            &config,
            &format!(
                "cloud_origin = \"https://cresix.example\"\nlocal_origin = \"http://127.0.0.1:8787\"\ninstallation_id = \"00000000-0000-4000-8000-000000000001\"\nbinding_id = \"00000000-0000-4000-8000-000000000002\"\nlocal_workspace_id = \"00000000-0000-4000-8000-000000000003\"\ncloud_credential_file = {:?}\nlocal_credential_file = {:?}\n",
                cloud, local
            ),
        );
        let loaded = LoadedConfig::load(&config).unwrap();
        assert_eq!(
            loaded.credentials.cloud().parsed().class,
            TokenClass::Connector
        );
        assert_eq!(loaded.request_timeout(), Duration::from_secs(30));

        let unknown = dir.path().join("unknown.toml");
        private_file(
            &unknown,
            &format!(
                "cloud_origin = \"https://cresix.example\"\nlocal_origin = \"http://127.0.0.1:8787\"\ninstallation_id = \"00000000-0000-4000-8000-000000000001\"\nbinding_id = \"00000000-0000-4000-8000-000000000002\"\nlocal_workspace_id = \"00000000-0000-4000-8000-000000000003\"\ncloud_credential_file = {:?}\nlocal_credential_file = {:?}\nsurprise = true\n",
                cloud, local
            ),
        );
        assert_eq!(
            LoadedConfig::load(&unknown).unwrap_err(),
            ConfigError::InvalidConfig
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_group_readable_files_symlinks_and_hardlinks() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        private_file(&path, "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert_eq!(load_credential(&path), Err(ConfigError::UnsafePermissions));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        let symlink = dir.path().join("link");
        std::os::unix::fs::symlink(&path, &symlink).unwrap();
        assert_eq!(load_credential(&symlink), Err(ConfigError::NotRegularFile));

        let hardlink = dir.path().join("hardlink");
        fs::hard_link(&path, &hardlink).unwrap();
        assert_eq!(load_credential(&path), Err(ConfigError::NotRegularFile));
    }

    #[test]
    fn credentials_have_bounded_safe_encoding() {
        let dir = tempfile::tempdir().unwrap();
        for (name, value) in [
            ("short", "short"),
            ("spaces", "xxxxxxxxxxxxxxxxxxxxxxxxxxxx xxx"),
            ("multi", "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\nsecond"),
            ("quote", "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\""),
        ] {
            let path = dir.path().join(name);
            private_file(&path, value);
            assert_eq!(load_credential(&path), Err(ConfigError::InvalidCredential));
        }
    }
}
