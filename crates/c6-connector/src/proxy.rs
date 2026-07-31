use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue, Method};
use reqwest::{Client, redirect::Policy};
use thiserror::Error;
use url::Url;

use crate::protocol::{HeaderField, MAX_RESPONSE_BODY_BYTES};

#[derive(Clone)]
pub struct FixedUpstream {
    origin: Url,
    client: Client,
}

#[derive(Debug)]
pub struct ProxyRequest {
    pub method: String,
    pub target: String,
    pub headers: Vec<HeaderField>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<HeaderField>,
    pub body: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProxyError {
    #[error("invalid relay request")]
    InvalidRequest,
    #[error("local C6 did not respond")]
    Unavailable,
    #[error("local C6 request timed out")]
    Timeout,
    #[error("local C6 response exceeded the limit")]
    ResponseTooLarge,
}

impl FixedUpstream {
    pub fn new(origin: Url, timeout: Duration) -> Result<Self, ProxyError> {
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|_| ProxyError::Unavailable)?;
        Ok(Self { origin, client })
    }

    pub async fn execute(&self, request: ProxyRequest) -> Result<ProxyResponse, ProxyError> {
        let method = validate_method(&request.method)?;
        let url = self.target_url(&request.target)?;
        let headers = filter_request_headers(request.headers)?;
        let response = self
            .client
            .request(method, url)
            .headers(headers)
            .body(request.body)
            .send()
            .await
            .map_err(classify_reqwest)?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BODY_BYTES)
        {
            return Err(ProxyError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let headers = response_headers(response.headers());
        let bytes = response.bytes().await.map_err(classify_reqwest)?;
        if bytes.len() as u64 > MAX_RESPONSE_BODY_BYTES {
            return Err(ProxyError::ResponseTooLarge);
        }
        Ok(ProxyResponse {
            status,
            headers,
            body: bytes.to_vec(),
        })
    }

    fn target_url(&self, target: &str) -> Result<Url, ProxyError> {
        if !target.starts_with('/')
            || target.starts_with("//")
            || target.contains('#')
            || target.bytes().any(|b| b.is_ascii_control())
        {
            return Err(ProxyError::InvalidRequest);
        }
        let mut value = self.origin.as_str().trim_end_matches('/').to_owned();
        value.push_str(target);
        let parsed = Url::parse(&value).map_err(|_| ProxyError::InvalidRequest)?;
        if parsed.scheme() != self.origin.scheme()
            || parsed.host_str() != self.origin.host_str()
            || parsed.port_or_known_default() != self.origin.port_or_known_default()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(ProxyError::InvalidRequest);
        }
        Ok(parsed)
    }
}

fn validate_method(value: &str) -> Result<Method, ProxyError> {
    match value {
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS" => {
            Method::from_bytes(value.as_bytes()).map_err(|_| ProxyError::InvalidRequest)
        }
        _ => Err(ProxyError::InvalidRequest),
    }
}

fn filter_request_headers(fields: Vec<HeaderField>) -> Result<HeaderMap, ProxyError> {
    let mut output = HeaderMap::new();
    for field in fields {
        let name: HeaderName = field.name.parse().map_err(|_| ProxyError::InvalidRequest)?;
        if blocked_header(&name) {
            continue;
        }
        let value = HeaderValue::from_str(&field.value).map_err(|_| ProxyError::InvalidRequest)?;
        output.append(name, value);
    }
    Ok(output)
}

fn response_headers(headers: &HeaderMap) -> Vec<HeaderField> {
    headers
        .iter()
        .filter(|(name, _)| !blocked_header(name))
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|value| HeaderField {
                name: name.as_str().to_owned(),
                value: value.to_owned(),
            })
        })
        .collect()
}

fn blocked_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | "content-length"
            | "forwarded"
            | "x-forwarded-for"
            | "x-forwarded-host"
            | "x-forwarded-proto"
            | "x-real-ip"
            | "x-c6-route"
            | "x-c6-installation"
            | "x-cresix-route"
            | "x-cresix-installation"
    ) || name.as_str().starts_with("x-c6-relay-")
}

fn classify_reqwest(error: reqwest::Error) -> ProxyError {
    if error.is_timeout() {
        ProxyError::Timeout
    } else {
        ProxyError::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_cannot_escape_fixed_origin() {
        let upstream = FixedUpstream::new(
            Url::parse("http://127.0.0.1:8787/").unwrap(),
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(upstream.target_url("/api/v1/status?ok=yes").is_ok());
        for target in [
            "https://evil.example/",
            "//evil.example/",
            "/ok#fragment",
            "/ok\nHost: evil.example",
        ] {
            assert_eq!(upstream.target_url(target), Err(ProxyError::InvalidRequest));
        }
    }

    #[test]
    fn methods_are_allowlisted() {
        for allowed in ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"] {
            assert!(validate_method(allowed).is_ok());
        }
        for denied in ["CONNECT", "TRACE", "get", "INVALID METHOD"] {
            assert_eq!(validate_method(denied), Err(ProxyError::InvalidRequest));
        }
    }

    #[test]
    fn removes_hop_forwarding_and_internal_headers() {
        let filtered = filter_request_headers(vec![
            HeaderField {
                name: "accept".into(),
                value: "application/json".into(),
            },
            HeaderField {
                name: "authorization".into(),
                value: "Bearer local-user-token".into(),
            },
            HeaderField {
                name: "host".into(),
                value: "evil.example".into(),
            },
            HeaderField {
                name: "forwarded".into(),
                value: "for=evil".into(),
            },
            HeaderField {
                name: "x-c6-relay-route".into(),
                value: "other".into(),
            },
            HeaderField {
                name: "connection".into(),
                value: "upgrade".into(),
            },
        ])
        .unwrap();
        assert_eq!(filtered.get("accept").unwrap(), "application/json");
        assert_eq!(
            filtered.get("authorization").unwrap(),
            "Bearer local-user-token"
        );
        assert!(filtered.get("host").is_none());
        assert!(filtered.get("forwarded").is_none());
        assert!(filtered.get("x-c6-relay-route").is_none());
        assert!(filtered.get("connection").is_none());
    }

    #[test]
    fn rejects_header_injection() {
        assert_eq!(
            filter_request_headers(vec![HeaderField {
                name: "x-ok".into(),
                value: "yes\r\nInjected: true".into(),
            }]),
            Err(ProxyError::InvalidRequest)
        );
    }
}
