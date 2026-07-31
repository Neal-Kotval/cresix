use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("c6.toml is not valid TOML: {0}")]
    InvalidToml(#[from] toml::de::Error),
    #[error("manifest version {0} is not supported; expected 1")]
    UnsupportedVersion(u32),
    #[error("service or job name {0:?} is declared more than once")]
    DuplicateName(String),
    #[error("web service {0:?} must use a port from 1 through 65535")]
    InvalidPort(String),
    #[error("cron job {0:?} must declare both schedule and timezone")]
    InvalidCron(String),
    #[error("agent job {0:?} must declare an agent configuration path")]
    MissingAgentConfig(String),
    #[error("secret reference {0:?} is not declared in [secrets]")]
    UnknownSecret(String),
    #[error("runtime name {0:?} must be 1-63 lowercase letters, digits, or hyphens")]
    InvalidName(String),
    #[error("runtime {0:?} must declare a command")]
    MissingCommand(String),
    #[error("job {0:?} must use a positive timeout")]
    InvalidTimeout(String),
    #[error("runtime {0:?} must request positive, finite CPU and non-zero memory")]
    InvalidResources(String),
    #[error("scheduled job {0:?} must declare schedule and timezone together")]
    PartialSchedule(String),
    #[error("dockerfile builds must declare a relative dockerfile path")]
    MissingDockerfile,
    #[error("manifest path {0:?} must be a safe relative path")]
    UnsafePath(String),
    #[error("secret name {0:?} must be a valid uppercase environment variable name")]
    InvalidSecretName(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub version: u32,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub postgres: Postgres,
    #[serde(default)]
    pub files: Files,
    #[serde(default)]
    pub secrets: BTreeMap<String, SecretDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Build {
    pub strategy: BuildStrategy,
    pub dockerfile: Option<String>,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            strategy: BuildStrategy::Auto,
            dockerfile: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildStrategy {
    #[default]
    Auto,
    Dockerfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub name: String,
    pub command: String,
    pub port: u16,
    #[serde(default = "default_health_path")]
    pub health_path: String,
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(default)]
    pub resources: Resources,
}

fn default_health_path() -> String {
    "/healthz".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub name: String,
    #[serde(default)]
    pub kind: JobKind,
    pub command: Option<String>,
    pub agent_config: Option<String>,
    pub schedule: Option<String>,
    pub timezone: Option<String>,
    #[serde(default)]
    pub concurrency: Concurrency,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(default)]
    pub repository_write: RepositoryWrite,
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub resources: Resources,
}

fn default_timeout() -> u32 {
    900
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    #[default]
    Command,
    Cron,
    Agent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concurrency {
    #[default]
    Forbid,
    Allow,
    Replace,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryWrite {
    #[default]
    None,
    Proposal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Resources {
    pub cpu: f32,
    pub memory_mb: u32,
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            cpu: 0.5,
            memory_mb: 512,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Postgres {
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Files {
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecretDeclaration {
    pub description: String,
}

impl ProjectManifest {
    pub fn parse(source: &str) -> Result<Self, ManifestError> {
        let manifest: Self = toml::from_str(source)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.version != 1 {
            return Err(ManifestError::UnsupportedVersion(self.version));
        }
        if self.build.strategy == BuildStrategy::Dockerfile {
            let path = self
                .build
                .dockerfile
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or(ManifestError::MissingDockerfile)?;
            validate_relative_path(path)?;
        }
        for secret in self.secrets.keys() {
            validate_secret_name(secret)?;
        }
        let mut names = BTreeSet::new();
        for service in &self.services {
            validate_name(&service.name)?;
            if !names.insert(service.name.as_str()) {
                return Err(ManifestError::DuplicateName(service.name.clone()));
            }
            if service.port == 0 {
                return Err(ManifestError::InvalidPort(service.name.clone()));
            }
            if service.command.trim().is_empty() {
                return Err(ManifestError::MissingCommand(service.name.clone()));
            }
            validate_resources(&service.name, &service.resources)?;
            self.validate_secrets(&service.secrets)?;
        }
        for job in &self.jobs {
            validate_name(&job.name)?;
            if !names.insert(job.name.as_str()) {
                return Err(ManifestError::DuplicateName(job.name.clone()));
            }
            if job.kind == JobKind::Cron && (job.schedule.is_none() || job.timezone.is_none()) {
                return Err(ManifestError::InvalidCron(job.name.clone()));
            }
            if job.schedule.is_some() != job.timezone.is_some() {
                return Err(ManifestError::PartialSchedule(job.name.clone()));
            }
            if job.kind == JobKind::Agent && job.agent_config.is_none() {
                return Err(ManifestError::MissingAgentConfig(job.name.clone()));
            }
            if matches!(job.kind, JobKind::Command | JobKind::Cron)
                && job
                    .command
                    .as_deref()
                    .is_none_or(|command| command.trim().is_empty())
            {
                return Err(ManifestError::MissingCommand(job.name.clone()));
            }
            if let Some(path) = &job.agent_config {
                validate_relative_path(path)?;
            }
            if job.timeout_seconds == 0 {
                return Err(ManifestError::InvalidTimeout(job.name.clone()));
            }
            validate_resources(&job.name, &job.resources)?;
            self.validate_secrets(&job.secrets)?;
        }
        Ok(())
    }

    fn validate_secrets(&self, references: &[String]) -> Result<(), ManifestError> {
        for name in references {
            if !self.secrets.contains_key(name) {
                return Err(ManifestError::UnknownSecret(name.clone()));
            }
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), ManifestError> {
    let bytes = name.as_bytes();
    let boundary_is_alphanumeric = bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric);
    if !name.is_empty()
        && name.len() <= 63
        && boundary_is_alphanumeric
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        Ok(())
    } else {
        Err(ManifestError::InvalidName(name.into()))
    }
}

fn validate_secret_name(name: &str) -> Result<(), ManifestError> {
    let mut bytes = name.bytes();
    let valid_start = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase() || byte == b'_');
    if valid_start
        && bytes.all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        Ok(())
    } else {
        Err(ManifestError::InvalidSecretName(name.into()))
    }
}

fn validate_resources(name: &str, resources: &Resources) -> Result<(), ManifestError> {
    if !resources.cpu.is_finite() || resources.cpu <= 0.0 || resources.memory_mb == 0 {
        Err(ManifestError::InvalidResources(name.into()))
    } else {
        Ok(())
    }
}

fn validate_relative_path(path: &str) -> Result<(), ManifestError> {
    let path_value = std::path::Path::new(path);
    let safe = !path.trim().is_empty()
        && !path_value.is_absolute()
        && path_value.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        });
    if safe {
        Ok(())
    } else {
        Err(ManifestError::UnsafePath(path.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_web_cron_and_agent_contract() {
        let source = r#"
version = 1

[postgres]
enabled = true

[secrets.OPENAI_API_KEY]
description = "Workspace model credential"

[[services]]
name = "web"
command = "./server"
port = 8080

[[jobs]]
name = "daily-triage"
kind = "agent"
agent_config = "agents/triage.toml"
schedule = "0 9 * * 1-5"
timezone = "America/New_York"
secrets = ["OPENAI_API_KEY"]
repository_write = "proposal"
"#;
        let manifest = ProjectManifest::parse(source).unwrap();
        assert_eq!(manifest.services[0].port, 8080);
        assert_eq!(manifest.jobs[0].kind, JobKind::Agent);
        assert!(manifest.postgres.enabled);
    }

    #[test]
    fn rejects_undeclared_secret() {
        let source = r#"
version = 1
[[services]]
name = "web"
command = "./server"
port = 8080
secrets = ["DATABASE_PASSWORD"]
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::UnknownSecret(_))
        ));
    }

    #[test]
    fn rejects_duplicate_runtime_names() {
        let source = r#"
version = 1
[[services]]
name = "web"
command = "./server"
port = 8080
[[jobs]]
name = "web"
command = "./job"
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::DuplicateName(_))
        ));
    }

    #[test]
    fn rejects_command_job_without_a_command() {
        let source = r#"
version = 1
[[jobs]]
name = "backup"
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::MissingCommand(name)) if name == "backup"
        ));
    }

    #[test]
    fn rejects_partial_schedule_configuration() {
        let source = r#"
version = 1
[[jobs]]
name = "backup"
command = "./backup"
schedule = "0 2 * * *"
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::PartialSchedule(_))
        ));
    }

    #[test]
    fn rejects_paths_that_escape_the_repository() {
        let source = r#"
version = 1
[[jobs]]
name = "agent"
kind = "agent"
agent_config = "../private.toml"
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::UnsafePath(_))
        ));
    }

    #[test]
    fn rejects_invalid_resource_limits() {
        let source = r#"
version = 1
[[services]]
name = "web"
command = "./web"
port = 8080
[services.resources]
cpu = 0.0
memory_mb = 512
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::InvalidResources(_))
        ));
    }

    #[test]
    fn dockerfile_build_requires_a_safe_path() {
        let missing = r#"
version = 1
[build]
strategy = "dockerfile"
"#;
        assert!(matches!(
            ProjectManifest::parse(missing),
            Err(ManifestError::MissingDockerfile)
        ));

        let valid = r#"
version = 1
[build]
strategy = "dockerfile"
dockerfile = "deploy/Dockerfile"
"#;
        assert!(ProjectManifest::parse(valid).is_ok());
    }

    #[test]
    fn rejects_names_that_are_unsafe_as_route_and_container_identifiers() {
        for name in [
            "",
            "Uppercase",
            "starts_underscore",
            "../escape",
            "trailing-",
        ] {
            let source = format!(
                r#"
version = 1
[[services]]
name = {name:?}
command = "./web"
port = 8080
"#
            );
            assert!(matches!(
                ProjectManifest::parse(&source),
                Err(ManifestError::InvalidName(_))
            ));
        }
    }

    #[test]
    fn rejects_unsafe_secret_environment_names() {
        let source = r#"
version = 1
[secrets.bad-name]
description = "invalid environment key"
"#;
        assert!(matches!(
            ProjectManifest::parse(source),
            Err(ManifestError::InvalidSecretName(_))
        ));
    }
}
