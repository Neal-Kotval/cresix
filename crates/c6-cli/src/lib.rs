//! Local state and command primitives for the C6 command-line tools.

pub mod config;
pub mod credential;

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    io,
    process::{Command, ExitStatus},
};

use c6_client::{Client, ClientError, Origin, ProjectSummary, WhoAmI};
use serde::Serialize;
use thiserror::Error;
use url::Url;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Usage(String),
    #[error("authentication is missing or invalid")]
    Authentication,
    #[error("operation forbidden")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("state conflict")]
    Conflict,
    #[error("network or TLS failure: {0}")]
    Network(String),
    #[error("server protocol mismatch")]
    Protocol,
    #[error("local configuration or credential failure: {0}")]
    Local(String),
    #[error("Git exited unsuccessfully")]
    GitFailure,
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Authentication => 10,
            Self::Forbidden => 11,
            Self::NotFound => 12,
            Self::Conflict => 13,
            Self::Network(_) => 20,
            Self::Protocol => 21,
            Self::Local(_) => 30,
            Self::GitFailure => 31,
            Self::Internal(_) => 1,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            Self::Authentication => "unauthenticated",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Network(_) => "network",
            Self::Protocol => "protocol_mismatch",
            Self::Local(_) => "local_state",
            Self::GitFailure => "git_failed",
            Self::Internal(_) => "internal",
        }
    }
}

impl From<ClientError> for AppError {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::InvalidOrigin | ClientError::InsecureOrigin => {
                Self::Usage(value.to_string())
            }
            ClientError::Unauthenticated => Self::Authentication,
            ClientError::Forbidden => Self::Forbidden,
            ClientError::NotFound => Self::NotFound,
            ClientError::Conflict => Self::Conflict,
            ClientError::Network(message) => Self::Network(message),
            ClientError::Protocol => Self::Protocol,
            ClientError::Api { .. } => Self::Protocol,
        }
    }
}

#[derive(Serialize)]
pub struct JsonSuccess<T: Serialize> {
    version: u8,
    ok: bool,
    data: T,
}

impl<T: Serialize> JsonSuccess<T> {
    pub fn new(data: T) -> Self {
        Self {
            version: 1,
            ok: true,
            data,
        }
    }
}

#[derive(Serialize)]
pub struct JsonFailure<'a> {
    version: u8,
    ok: bool,
    error: JsonError<'a>,
}

#[derive(Serialize)]
struct JsonError<'a> {
    code: &'a str,
    message: String,
}

impl<'a> JsonFailure<'a> {
    pub fn new(error: &'a AppError) -> Self {
        Self {
            version: 1,
            ok: false,
            error: JsonError {
                code: error.code(),
                message: error.to_string(),
            },
        }
    }
}

pub fn selected_server<'a>(
    config: &'a config::Config,
    requested: Option<&str>,
) -> Result<(&'a str, &'a config::Server), AppError> {
    let alias = requested
        .or(config.default_server.as_deref())
        .ok_or_else(|| AppError::Local("no server selected; use `c6 server add`".into()))?;
    let (configured_alias, server) = config
        .servers
        .get_key_value(alias)
        .ok_or_else(|| AppError::Local(format!("server alias `{alias}` is not configured")))?;
    Ok((configured_alias.as_str(), server))
}

pub fn authenticated_client(
    paths: &config::Paths,
    requested: Option<&str>,
) -> Result<(String, Client), AppError> {
    let config = config::Config::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
    let (alias, server) = selected_server(&config, requested)?;
    let credentials =
        credential::CredentialStore::load(paths).map_err(|e| AppError::Local(e.to_string()))?;
    let token = credentials
        .api_token(alias)
        .ok_or(AppError::Authentication)?
        .expose()
        .to_owned();
    let origin = Origin::parse(&server.base_url, server.allow_http_localhost)?;
    Ok((alias.to_owned(), Client::new(origin, Some(token))?))
}

pub fn resolve_project(
    client: &Client,
    reference: &str,
) -> Result<(WhoAmI, ProjectSummary), AppError> {
    let (workspace_slug, project_slug) = parse_project_ref(reference)?;
    let who = client.whoami()?;
    let workspace = who
        .workspaces
        .iter()
        .find(|w| w.slug == workspace_slug)
        .ok_or(AppError::NotFound)?;
    let projects = client.projects()?;
    let project = projects
        .projects
        .into_iter()
        .find(|p| p.workspace_id == workspace.id && p.slug == project_slug)
        .ok_or(AppError::NotFound)?;
    Ok((who, project))
}

pub fn parse_project_ref(value: &str) -> Result<(&str, &str), AppError> {
    let mut parts = value.split('/');
    let (Some(workspace), Some(project), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(AppError::Usage(
            "project must be `<workspace>/<project>`".into(),
        ));
    };
    if !valid_slug(workspace) || !valid_slug(project) {
        return Err(AppError::Usage(
            "workspace and project must be lowercase URL-safe slugs".into(),
        ));
    }
    Ok((workspace, project))
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

pub fn run_git<I>(args: I, machine_mode: bool) -> Result<ExitStatus, AppError>
where
    I: IntoIterator<Item = OsString>,
{
    run_git_program(OsStr::new("git"), args, machine_mode)
}

fn run_git_program<I>(program: &OsStr, args: I, machine_mode: bool) -> Result<ExitStatus, AppError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut command = Command::new(program);
    command.args(args);
    if machine_mode {
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "Never");
        #[cfg(unix)]
        command.env("GIT_ASKPASS", "/usr/bin/false");
    }
    command
        .status()
        .map_err(|error| AppError::Local(format!("could not execute Git: {error}")))
}

pub fn git_environment() -> Result<BTreeMap<&'static str, String>, AppError> {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .map_err(|error| AppError::Local(format!("could not execute Git: {error}")))?;
    if !output.status.success() {
        return Err(AppError::GitFailure);
    }
    let version = String::from_utf8(output.stdout)
        .map_err(|_| AppError::Protocol)?
        .trim()
        .to_owned();
    Ok(BTreeMap::from([("version", version)]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCredentialConfig {
    pub helper_key: String,
    pub use_http_path_key: String,
}

/// Builds an exact URL subsection for a credential-free C6 smart-HTTP URL.
///
/// The caller must install an empty value for `helper_key` before adding C6;
/// in Git configuration an empty helper resets all inherited helper entries.
pub fn git_credential_config(remote_url: &str) -> Result<GitCredentialConfig, AppError> {
    let parsed = Url::parse(remote_url).map_err(|_| AppError::Protocol)?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !c6_client::valid_git_path(parsed.path())
    {
        return Err(AppError::Protocol);
    }
    Ok(GitCredentialConfig {
        helper_key: format!("credential.{remote_url}.helper"),
        use_http_path_key: format!("credential.{remote_url}.useHttpPath"),
    })
}

pub fn read_secret_stdin() -> Result<String, AppError> {
    use io::Read;
    let mut bytes = Vec::new();
    io::stdin()
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::Local(format!("could not read token: {error}")))?;
    if bytes.len() > 16 * 1024 {
        return Err(AppError::Usage("token input is too large".into()));
    }
    let token =
        String::from_utf8(bytes).map_err(|_| AppError::Usage("token must be UTF-8".into()))?;
    let token = token.trim().to_owned();
    if !token.starts_with("c6c_v1_") || token.chars().any(char::is_whitespace) {
        return Err(AppError::Authentication);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_reference_is_exact() {
        assert_eq!(
            parse_project_ref("paper-street/weeknote").unwrap(),
            ("paper-street", "weeknote")
        );
        for value in ["paper-street", "a/b/c", "A/b", "a/../b", "-a/b"] {
            assert!(parse_project_ref(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn error_exit_contract_is_stable() {
        assert_eq!(AppError::Authentication.exit_code(), 10);
        assert_eq!(AppError::Forbidden.exit_code(), 11);
        assert_eq!(AppError::NotFound.exit_code(), 12);
        assert_eq!(AppError::Conflict.exit_code(), 13);
        assert_eq!(AppError::Protocol.exit_code(), 21);
        assert_eq!(AppError::GitFailure.exit_code(), 31);
    }

    #[test]
    fn credential_config_is_exact_and_credential_free() {
        let config =
            git_credential_config("https://c6.example/git/paper-street/weeknote.git").unwrap();
        assert_eq!(
            config.helper_key,
            "credential.https://c6.example/git/paper-street/weeknote.git.helper"
        );
        assert_eq!(
            config.use_http_path_key,
            "credential.https://c6.example/git/paper-street/weeknote.git.useHttpPath"
        );
        for unsafe_url in [
            "https://token@c6.example/git/a/b.git",
            "https://c6.example/git/a/b.git?token=x",
            "https://c6.example/git/a/../b.git",
            "ssh://c6.example/git/a/b.git",
        ] {
            assert!(
                git_credential_config(unsafe_url).is_err(),
                "accepted {unsafe_url}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn json_git_delegation_disables_all_supported_prompts() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let fake_git = temp.path().join("git");
        fs::write(
            &fake_git,
            concat!(
                "#!/bin/sh\n",
                "test \"$GIT_TERMINAL_PROMPT\" = 0 || exit 90\n",
                "test \"$GCM_INTERACTIVE\" = Never || exit 91\n",
                "test \"$GIT_ASKPASS\" = /usr/bin/false || exit 92\n",
                "exit 31\n",
            ),
        )
        .unwrap();
        fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o700)).unwrap();
        let status =
            run_git_program(fake_git.as_os_str(), [OsString::from("clone")], true).unwrap();
        assert_eq!(status.code(), Some(31));
    }

    #[cfg(unix)]
    #[test]
    fn empty_url_scoped_helper_blocks_inherited_helper_store() {
        use std::{
            fs,
            io::Write,
            os::unix::fs::PermissionsExt,
            process::{Command, Stdio},
        };

        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let repository = temp.path().join("repository");
        fs::create_dir(&home).unwrap();
        fs::create_dir(&repository).unwrap();
        let inherited_marker = temp.path().join("inherited-called");
        let c6_marker = temp.path().join("c6-called");
        let inherited_helper = temp.path().join("inherited-helper");
        let c6_helper = temp.path().join("c6-helper");
        for (helper, marker) in [
            (&inherited_helper, &inherited_marker),
            (&c6_helper, &c6_marker),
        ] {
            fs::write(
                helper,
                format!("#!/bin/sh\n: > '{}'\ncat >/dev/null\n", marker.display()),
            )
            .unwrap();
            fs::set_permissions(helper, fs::Permissions::from_mode(0o700)).unwrap();
        }

        let git = |args: &[&str], directory: &std::path::Path| {
            Command::new("git")
                .args(args)
                .current_dir(directory)
                .env("HOME", &home)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .status()
                .unwrap()
        };
        assert!(git(&["init", "--quiet"], &repository).success());
        assert!(
            git(
                &[
                    "config",
                    "--global",
                    "--add",
                    "credential.helper",
                    inherited_helper.to_str().unwrap(),
                ],
                &repository,
            )
            .success()
        );

        let remote = "https://c6.example/git/paper-street/weeknote.git";
        let config = git_credential_config(remote).unwrap();
        assert!(
            git(
                &["config", "--local", "--replace-all", &config.helper_key, ""],
                &repository,
            )
            .success()
        );
        assert!(
            git(
                &[
                    "config",
                    "--local",
                    "--add",
                    &config.helper_key,
                    c6_helper.to_str().unwrap(),
                ],
                &repository,
            )
            .success()
        );
        assert!(
            git(
                &["config", "--local", &config.use_http_path_key, "true"],
                &repository,
            )
            .success()
        );

        let mut child = Command::new("git")
            .args(["credential", "approve"])
            .current_dir(&repository)
            .env("HOME", &home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                b"protocol=https\nhost=c6.example\npath=git/paper-street/weeknote.git\nusername=c6\npassword=regression-only\n\n",
            )
            .unwrap();
        assert!(child.wait().unwrap().success());
        assert!(c6_marker.exists(), "the C6-scoped helper was not invoked");
        assert!(
            !inherited_marker.exists(),
            "an inherited helper received C6 credential store input"
        );
    }
}
