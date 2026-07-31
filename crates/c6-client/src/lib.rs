//! Typed, transport-only client for a C6 authority.
//!
//! This crate deliberately owns no local persistence and never follows HTTP
//! redirects. Callers must keep bearer credentials out of URLs and logs.

use std::{fmt, io::Read, time::Duration};

use reqwest::{StatusCode, blocking::Response, redirect::Policy};
use serde::de::DeserializeOwned;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;

// Public wire contracts are owned by c6-core. Compatibility aliases retain
// the initial client-facing names without maintaining a second schema here.
pub use c6_core::{
    ApiError as ApiErrorBody, BootstrapStatusResponse as ServerStatus,
    CliProjectListResponse as ProjectList, CliProjectSummary as ProjectSummary,
    CliServerSummary as ServerSummary, CliUserSummary as UserSummary, CliWhoAmIResponse as WhoAmI,
    CliWorkspaceSummary as WorkspaceSummary, CreateCredentialRequest, CreateCredentialResponse,
    CredentialListResponse, CredentialMetadata, CredentialResourceRestriction, CredentialScope,
    CredentialType, ProjectRemoteResponse as ProjectRemote,
    RemoteTransportCapabilities as TransportCapabilities,
};

/// Validated C6 server origin. Its display form never includes a credential.
#[derive(Clone, PartialEq, Eq)]
pub struct Origin(Url);

impl Origin {
    pub fn parse(input: &str, allow_http_localhost: bool) -> Result<Self, ClientError> {
        let mut url = Url::parse(input).map_err(|_| ClientError::InvalidOrigin)?;
        let is_https = url.scheme() == "https";
        let is_loopback_http = url.scheme() == "http"
            && allow_http_localhost
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
        if !is_https && !is_loopback_http {
            return Err(ClientError::InsecureOrigin);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ClientError::InvalidOrigin);
        }
        if url.query().is_some() || url.fragment().is_some() || url.path() != "/" {
            return Err(ClientError::InvalidOrigin);
        }
        if url.host_str().is_none() {
            return Err(ClientError::InvalidOrigin);
        }
        url.set_path("");
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str().trim_end_matches('/')
    }

    pub fn matches_url(&self, candidate: &Url) -> bool {
        self.0.scheme() == candidate.scheme()
            && self.0.host_str() == candidate.host_str()
            && self.0.port_or_known_default() == candidate.port_or_known_default()
    }

    fn endpoint(&self, path: &str) -> Result<Url, ClientError> {
        debug_assert!(path.starts_with('/'));
        self.0.join(path).map_err(|_| ClientError::Protocol)
    }
}

impl fmt::Debug for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Origin").field(&self.as_str()).finish()
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("server URL must be an origin without a path, query, fragment, or credentials")]
    InvalidOrigin,
    #[error("server URL must use HTTPS (loopback HTTP requires explicit opt-in)")]
    InsecureOrigin,
    #[error("authentication is missing or invalid")]
    Unauthenticated,
    #[error("the server denied this operation")]
    Forbidden,
    #[error("the requested resource was not found")]
    NotFound,
    #[error("the operation conflicts with existing state")]
    Conflict,
    #[error("network or TLS request failed: {0}")]
    Network(String),
    #[error("server protocol mismatch")]
    Protocol,
    #[error("server returned {status}: {body:?}")]
    Api { status: u16, body: ApiErrorBody },
}

impl ClientError {
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api { body, .. } => body.request_id.as_deref(),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Client {
    origin: Origin,
    bearer: Option<String>,
    http: reqwest::blocking::Client,
}

impl Client {
    pub fn new(origin: Origin, bearer: Option<String>) -> Result<Self, ClientError> {
        let http = reqwest::blocking::Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| ClientError::Network(redact(error.to_string(), bearer.as_deref())))?;
        Ok(Self {
            origin,
            bearer,
            http,
        })
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    pub fn whoami(&self) -> Result<WhoAmI, ClientError> {
        self.get("/api/v1/cli/whoami")
    }

    pub fn status(&self) -> Result<ServerStatus, ClientError> {
        self.get("/api/v1/status")
    }

    pub fn projects(&self) -> Result<ProjectList, ClientError> {
        self.get("/api/v1/projects")
    }

    pub fn project_remote(&self, project_id: &Uuid) -> Result<ProjectRemote, ClientError> {
        let remote: ProjectRemote = self.get(&format!("/api/v1/projects/{project_id}/remote"))?;
        if &remote.project_id != project_id {
            return Err(ClientError::Protocol);
        }
        let clone = Url::parse(&remote.clone_url).map_err(|_| ClientError::Protocol)?;
        if !self.origin.matches_url(&clone)
            || clone.username() != ""
            || clone.password().is_some()
            || clone.query().is_some()
            || clone.fragment().is_some()
            || !valid_git_path(clone.path())
        {
            return Err(ClientError::Protocol);
        }
        Ok(remote)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let url = self.origin.endpoint(path)?;
        let mut request = self.http.get(url).header("Accept", "application/json");
        if let Some(token) = &self.bearer {
            request = request.bearer_auth(token);
        }
        let response = request.send().map_err(|error| {
            ClientError::Network(redact(error.to_string(), self.bearer.as_deref()))
        })?;
        decode(response)
    }
}

pub fn valid_git_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("/git/") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(".git") else {
        return false;
    };
    let mut parts = rest.split('/');
    let (Some(workspace), Some(project), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [workspace, project].into_iter().all(valid_slug)
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

fn decode<T: DeserializeOwned>(response: Response) -> Result<T, ClientError> {
    let status = response.status();
    if status.is_redirection() {
        return Err(ClientError::Protocol);
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES)
    {
        return Err(ClientError::Protocol);
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ClientError::Network(error.to_string()))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(ClientError::Protocol);
    }
    if status.is_success() {
        return serde_json::from_slice(&bytes).map_err(|_| ClientError::Protocol);
    }
    let body = serde_json::from_slice::<c6_core::ApiErrorResponse>(&bytes)
        .map_err(|_| ClientError::Protocol)?
        .error;
    match status {
        StatusCode::UNAUTHORIZED => Err(ClientError::Unauthenticated),
        StatusCode::FORBIDDEN => Err(ClientError::Forbidden),
        StatusCode::NOT_FOUND => Err(ClientError::NotFound),
        StatusCode::CONFLICT => Err(ClientError::Conflict),
        _ => Err(ClientError::Api {
            status: status.as_u16(),
            body,
        }),
    }
}

fn redact(mut message: String, secret: Option<&str>) -> String {
    if let Some(secret) = secret.filter(|value| !value.is_empty()) {
        message = message.replace(secret, "[REDACTED]");
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::GET, MockServer};

    #[test]
    fn origin_is_deny_by_default() {
        for value in [
            "http://example.com",
            "https://u:p@example.com",
            "https://example.com/a",
            "https://example.com?x=1",
            "https://example.com/#x",
            "file:///tmp/x",
        ] {
            assert!(Origin::parse(value, false).is_err(), "accepted {value}");
        }
        assert!(Origin::parse("http://localhost:8787", false).is_err());
        assert_eq!(
            Origin::parse("http://localhost:8787", true)
                .unwrap()
                .as_str(),
            "http://localhost:8787"
        );
        assert_eq!(
            Origin::parse("https://example.com", false)
                .unwrap()
                .as_str(),
            "https://example.com"
        );
    }

    #[test]
    fn git_path_is_exact() {
        assert!(valid_git_path("/git/paper-street/weeknote.git"));
        for path in [
            "/git/x.git",
            "/git/a/b",
            "/git/a/../b.git",
            "/git/A/b.git",
            "/git/a/b.git/x",
        ] {
            assert!(!valid_git_path(path), "accepted {path}");
        }
    }

    #[test]
    fn authorization_is_injected_and_response_is_typed() {
        let server = MockServer::start();
        let server_id = "30000000-0000-4000-8000-000000000001";
        let user_id = "30000000-0000-4000-8000-000000000002";
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/v1/cli/whoami")
                .header("authorization", "Bearer c6c_v1_public_secret");
            then.status(200).json_body_obj(&serde_json::json!({
                "server":{"id":server_id,"name":"C6"},
                "user":{"id":user_id,"displayName":"Neal"}, "workspaces":[]
            }));
        });
        let origin = Origin::parse(&server.base_url(), true).unwrap();
        let who = Client::new(origin, Some("c6c_v1_public_secret".into()))
            .unwrap()
            .whoami()
            .unwrap();
        assert_eq!(who.server.id, Uuid::parse_str(server_id).unwrap());
        mock.assert();
    }

    #[test]
    fn remote_must_stay_on_pinned_origin() {
        let server = MockServer::start();
        let project_id = Uuid::parse_str("40000000-0000-4000-8000-000000000001").unwrap();
        server.mock(|when, then| {
            when.method(GET)
                .path(format!("/api/v1/projects/{project_id}/remote"));
            then.status(200).json_body_obj(&serde_json::json!({
                "projectId":project_id, "cloneUrl":"https://evil.example/git/a/b.git",
                "capabilities":{"fetch":true,"push":false}
            }));
        });
        let client = Client::new(Origin::parse(&server.base_url(), true).unwrap(), None).unwrap();
        assert!(matches!(
            client.project_remote(&project_id),
            Err(ClientError::Protocol)
        ));
    }

    #[test]
    fn canonical_responses_reject_unknown_fields_and_invalid_roles() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/cli/whoami");
            then.status(200).json_body_obj(&serde_json::json!({
                "server":{
                    "id":"30000000-0000-4000-8000-000000000001",
                    "name":"C6"
                },
                "user":{
                    "id":"30000000-0000-4000-8000-000000000002",
                    "displayName":"Neal",
                    "serverAdministrator":true
                },
                "workspaces":[]
            }));
        });
        let client = Client::new(Origin::parse(&server.base_url(), true).unwrap(), None).unwrap();
        assert!(matches!(client.whoami(), Err(ClientError::Protocol)));

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/projects");
            then.status(200).json_body_obj(&serde_json::json!({
                "projects":[{
                    "id":"40000000-0000-4000-8000-000000000001",
                    "workspaceId":"40000000-0000-4000-8000-000000000002",
                    "slug":"weeknote",
                    "name":"Weeknote",
                    "description":"",
                    "defaultBranch":"main",
                    "headSha":"0123456789012345678901234567890123456789",
                    "publishedSha":null,
                    "role":"administrator",
                    "updatedAt":"2026-07-31T12:34:56Z"
                }]
            }));
        });
        let client = Client::new(Origin::parse(&server.base_url(), true).unwrap(), None).unwrap();
        assert!(matches!(client.projects(), Err(ClientError::Protocol)));
    }

    #[test]
    fn malformed_or_noncanonical_error_envelopes_are_protocol_errors() {
        for body in [
            serde_json::json!({"code":"internal","message":"no envelope"}),
            serde_json::json!({
                "error":{"code":"internal","message":"failed","secret":"leak"}
            }),
            serde_json::json!({
                "error":{"code":"made_up","message":"failed"}
            }),
        ] {
            let server = MockServer::start();
            server.mock(|when, then| {
                when.method(GET).path("/api/v1/status");
                then.status(500).json_body_obj(&body);
            });
            let client =
                Client::new(Origin::parse(&server.base_url(), true).unwrap(), None).unwrap();
            assert!(matches!(client.status(), Err(ClientError::Protocol)));
        }
    }
}
