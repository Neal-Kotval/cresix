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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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
        let mut names = BTreeSet::new();
        for service in &self.services {
            if !names.insert(service.name.as_str()) {
                return Err(ManifestError::DuplicateName(service.name.clone()));
            }
            if service.port == 0 {
                return Err(ManifestError::InvalidPort(service.name.clone()));
            }
            self.validate_secrets(&service.secrets)?;
        }
        for job in &self.jobs {
            if !names.insert(job.name.as_str()) {
                return Err(ManifestError::DuplicateName(job.name.clone()));
            }
            if job.kind == JobKind::Cron && (job.schedule.is_none() || job.timezone.is_none()) {
                return Err(ManifestError::InvalidCron(job.name.clone()));
            }
            if job.kind == JobKind::Agent && job.agent_config.is_none() {
                return Err(ManifestError::MissingAgentConfig(job.name.clone()));
            }
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
}
