use crate::{SecretToken, TokenClass};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
use thiserror::Error;
use uuid::Uuid;

pub const RELAY_PROTOCOL_VERSION: u16 = 1;
pub const RELAY_SUBPROTOCOL: &str = "c6-relay-v1";
pub const MAX_BODY_CHUNK_BYTES: usize = 64 * 1024;
pub const MAX_REQUEST_BODY_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_RESPONSE_BODY_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CONCURRENT_REQUESTS: usize = 32;
pub const MAX_HEADER_COUNT: usize = 128;
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_TARGET_BYTES: usize = 8 * 1024;
pub const MAX_REQUEST_IDS_PER_SESSION: usize = 4_096;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct HttpMethod(String);

impl HttpMethod {
    pub fn new(value: impl Into<String>) -> Result<Self, RelayValidationError> {
        let value = value.into();
        if !matches!(
            value.as_str(),
            "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
        ) {
            return Err(RelayValidationError::InvalidMethod);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HttpMethod").field(&self.0).finish()
    }
}
impl Serialize for HttpMethod {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for HttpMethod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeaderField {
    pub name: String,
    pub value: String,
}

impl HeaderField {
    pub fn validate(&self) -> Result<(), RelayValidationError> {
        if self.name.is_empty()
            || !self.name.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || b"!#$%&'*+-.^_`|~".contains(&byte)
            })
        {
            return Err(RelayValidationError::InvalidHeader);
        }
        if self
            .value
            .bytes()
            .any(|byte| (byte.is_ascii_control() && byte != b'\t') || byte == 0x7f)
        {
            return Err(RelayValidationError::InvalidHeader);
        }
        if is_forbidden_header(&self.name) {
            return Err(RelayValidationError::ForbiddenHeader);
        }
        Ok(())
    }
}

pub fn is_forbidden_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "host"
            | "forwarded"
            | "x-forwarded-for"
            | "x-forwarded-host"
            | "x-forwarded-proto"
            | "x-real-ip"
            | "x-c6-route"
            | "x-c6-installation"
            | "x-cresix-route"
            | "x-cresix-installation"
    )
}

fn validate_headers(headers: &[HeaderField]) -> Result<(), RelayValidationError> {
    if headers.len() > MAX_HEADER_COUNT {
        return Err(RelayValidationError::HeadersTooLarge);
    }
    let mut size = 0usize;
    for header in headers {
        header.validate()?;
        size = size
            .saturating_add(header.name.len())
            .saturating_add(header.value.len());
        if size > MAX_HEADER_BYTES {
            return Err(RelayValidationError::HeadersTooLarge);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientHelloFrame {
    pub protocol_version: u16,
    pub connector_token: SecretToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerReadyFrame {
    pub installation_id: Uuid,
    pub generation: u64,
    pub max_concurrent_requests: u16,
    pub max_chunk_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestStartFrame {
    pub request_id: Uuid,
    pub method: HttpMethod,
    /// Origin-relative path and optional query. It cannot select an upstream.
    pub target: String,
    pub headers: Vec<HeaderField>,
    pub deadline_unix_ms: u64,
}

impl RequestStartFrame {
    pub fn validate(&self) -> Result<(), RelayValidationError> {
        if self.target.is_empty()
            || self.target.len() > MAX_TARGET_BYTES
            || !self.target.starts_with('/')
            || self.target.starts_with("//")
            || self.target.contains('#')
            || self.target.contains("://")
            || self.target.contains('\\')
            || self.target.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(RelayValidationError::InvalidTarget);
        }
        if self.deadline_unix_ms == 0 {
            return Err(RelayValidationError::InvalidDeadline);
        }
        validate_headers(&self.headers)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestIdFrame {
    pub request_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponseStartFrame {
    pub request_id: Uuid,
    pub status: u16,
    pub headers: Vec<HeaderField>,
}
impl ResponseStartFrame {
    pub fn validate(&self) -> Result<(), RelayValidationError> {
        if !(100..=599).contains(&self.status) {
            return Err(RelayValidationError::InvalidStatus);
        }
        validate_headers(&self.headers)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayFailureCode {
    BadGateway,
    Timeout,
    Cancelled,
    Protocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestFailedFrame {
    pub request_id: Uuid,
    pub code: RelayFailureCode,
    /// Bounded generic description; implementations must not insert upstream
    /// headers, paths, bodies, credentials, or internal error chains.
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeepaliveFrame {
    pub nonce: u64,
}

/// Strict JSON control frames. The nested `data` object denies unknown fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RelayControlFrame {
    ClientHello(ClientHelloFrame),
    ServerReady(ServerReadyFrame),
    RequestStart(RequestStartFrame),
    RequestEnd(RequestIdFrame),
    ResponseStart(ResponseStartFrame),
    ResponseEnd(RequestIdFrame),
    Cancel(RequestIdFrame),
    RequestFailed(RequestFailedFrame),
    Ping(KeepaliveFrame),
    Pong(KeepaliveFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayBodyKind {
    RequestChunk,
    ResponseChunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayBodyFrame {
    pub kind: RelayBodyKind,
    pub request_id: Uuid,
    pub sequence: u32,
    pub payload: Vec<u8>,
}

impl RelayBodyFrame {
    const HEADER_BYTES: usize = 22;

    pub fn encode(&self) -> Result<Vec<u8>, RelayValidationError> {
        if self.payload.len() > MAX_BODY_CHUNK_BYTES {
            return Err(RelayValidationError::ChunkTooLarge);
        }
        let mut output = Vec::with_capacity(Self::HEADER_BYTES + self.payload.len());
        output.push(RELAY_PROTOCOL_VERSION as u8);
        output.push(match self.kind {
            RelayBodyKind::RequestChunk => 1,
            RelayBodyKind::ResponseChunk => 2,
        });
        output.extend_from_slice(self.request_id.as_bytes());
        output.extend_from_slice(&self.sequence.to_be_bytes());
        output.extend_from_slice(&self.payload);
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<Self, RelayValidationError> {
        if input.len() < Self::HEADER_BYTES {
            return Err(RelayValidationError::MalformedBodyFrame);
        }
        if input.len() - Self::HEADER_BYTES > MAX_BODY_CHUNK_BYTES {
            return Err(RelayValidationError::ChunkTooLarge);
        }
        if input[0] != RELAY_PROTOCOL_VERSION as u8 {
            return Err(RelayValidationError::UnsupportedProtocol);
        }
        let kind = match input[1] {
            1 => RelayBodyKind::RequestChunk,
            2 => RelayBodyKind::ResponseChunk,
            _ => return Err(RelayValidationError::MalformedBodyFrame),
        };
        let request_id = Uuid::from_slice(&input[2..18])
            .map_err(|_| RelayValidationError::MalformedBodyFrame)?;
        let sequence = u32::from_be_bytes(input[18..22].try_into().expect("fixed slice"));
        Ok(Self {
            kind,
            request_id,
            sequence,
            payload: input[22..].to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RelayValidationError {
    #[error("unsupported relay protocol")]
    UnsupportedProtocol,
    #[error("invalid HTTP method")]
    InvalidMethod,
    #[error("invalid request target")]
    InvalidTarget,
    #[error("invalid request deadline")]
    InvalidDeadline,
    #[error("invalid HTTP status")]
    InvalidStatus,
    #[error("invalid header")]
    InvalidHeader,
    #[error("forbidden transport or routing header")]
    ForbiddenHeader,
    #[error("headers exceed protocol limits")]
    HeadersTooLarge,
    #[error("body chunk exceeds protocol limit")]
    ChunkTooLarge,
    #[error("body exceeds protocol limit")]
    BodyTooLarge,
    #[error("malformed binary body frame")]
    MalformedBodyFrame,
    #[error("illegal relay state transition")]
    IllegalTransition,
    #[error("duplicate or reused request identifier")]
    DuplicateRequest,
    #[error("unknown request identifier")]
    UnknownRequest,
    #[error("request concurrency limit reached")]
    ConcurrencyLimit,
    #[error("request identifier budget exhausted")]
    RequestIdBudgetExhausted,
    #[error("body chunk sequence is not monotonic")]
    InvalidSequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandshakeState {
    AwaitingHello,
    AwaitingReady,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowState {
    RequestBody { next_sequence: u32, bytes: u64 },
    AwaitingResponse,
    ResponseBody { next_sequence: u32, bytes: u64 },
}

/// Transport-neutral session validator used by both relay and connector.
/// Call `observe_control` / `observe_body` before acting on an incoming frame.
#[derive(Debug)]
pub struct RelaySessionState {
    handshake: HandshakeState,
    flows: HashMap<Uuid, FlowState>,
    seen_request_ids: HashSet<Uuid>,
}

impl Default for RelaySessionState {
    fn default() -> Self {
        Self {
            handshake: HandshakeState::AwaitingHello,
            flows: HashMap::new(),
            seen_request_ids: HashSet::new(),
        }
    }
}

impl RelaySessionState {
    pub fn is_ready(&self) -> bool {
        self.handshake == HandshakeState::Ready
    }
    pub fn in_flight(&self) -> usize {
        self.flows.len()
    }

    pub fn observe_control(
        &mut self,
        frame: &RelayControlFrame,
    ) -> Result<(), RelayValidationError> {
        match frame {
            RelayControlFrame::ClientHello(frame) => {
                if self.handshake != HandshakeState::AwaitingHello {
                    return Err(RelayValidationError::IllegalTransition);
                }
                if frame.protocol_version != RELAY_PROTOCOL_VERSION {
                    return Err(RelayValidationError::UnsupportedProtocol);
                }
                if frame.connector_token.parsed().class != TokenClass::Connector {
                    return Err(RelayValidationError::IllegalTransition);
                }
                self.handshake = HandshakeState::AwaitingReady;
            }
            RelayControlFrame::ServerReady(frame) => {
                if self.handshake != HandshakeState::AwaitingReady {
                    return Err(RelayValidationError::IllegalTransition);
                }
                if frame.generation == 0
                    || frame.max_concurrent_requests == 0
                    || frame.max_concurrent_requests as usize > MAX_CONCURRENT_REQUESTS
                    || frame.max_chunk_bytes as usize > MAX_BODY_CHUNK_BYTES
                {
                    return Err(RelayValidationError::IllegalTransition);
                }
                self.handshake = HandshakeState::Ready;
            }
            _ if self.handshake != HandshakeState::Ready => {
                return Err(RelayValidationError::IllegalTransition);
            }
            RelayControlFrame::RequestStart(frame) => {
                frame.validate()?;
                if self.seen_request_ids.contains(&frame.request_id) {
                    return Err(RelayValidationError::DuplicateRequest);
                }
                if self.flows.len() >= MAX_CONCURRENT_REQUESTS {
                    return Err(RelayValidationError::ConcurrencyLimit);
                }
                if self.seen_request_ids.len() >= MAX_REQUEST_IDS_PER_SESSION {
                    return Err(RelayValidationError::RequestIdBudgetExhausted);
                }
                self.seen_request_ids.insert(frame.request_id);
                self.flows.insert(
                    frame.request_id,
                    FlowState::RequestBody {
                        next_sequence: 0,
                        bytes: 0,
                    },
                );
            }
            RelayControlFrame::RequestEnd(frame) => {
                let flow = self
                    .flows
                    .get_mut(&frame.request_id)
                    .ok_or(RelayValidationError::UnknownRequest)?;
                if !matches!(flow, FlowState::RequestBody { .. }) {
                    return Err(RelayValidationError::IllegalTransition);
                }
                *flow = FlowState::AwaitingResponse;
            }
            RelayControlFrame::ResponseStart(frame) => {
                frame.validate()?;
                let flow = self
                    .flows
                    .get_mut(&frame.request_id)
                    .ok_or(RelayValidationError::UnknownRequest)?;
                if *flow != FlowState::AwaitingResponse {
                    return Err(RelayValidationError::IllegalTransition);
                }
                *flow = FlowState::ResponseBody {
                    next_sequence: 0,
                    bytes: 0,
                };
            }
            RelayControlFrame::ResponseEnd(frame) => {
                let flow = self
                    .flows
                    .get(&frame.request_id)
                    .ok_or(RelayValidationError::UnknownRequest)?;
                if !matches!(flow, FlowState::ResponseBody { .. }) {
                    return Err(RelayValidationError::IllegalTransition);
                }
                self.flows.remove(&frame.request_id);
            }
            RelayControlFrame::Cancel(frame) => {
                if self.flows.remove(&frame.request_id).is_none() {
                    return Err(RelayValidationError::UnknownRequest);
                }
            }
            RelayControlFrame::RequestFailed(frame) => {
                if frame.message.len() > 200 || frame.message.chars().any(char::is_control) {
                    return Err(RelayValidationError::IllegalTransition);
                }
                if self.flows.remove(&frame.request_id).is_none() {
                    return Err(RelayValidationError::UnknownRequest);
                }
            }
            RelayControlFrame::Ping(_) | RelayControlFrame::Pong(_) => {}
        }
        Ok(())
    }

    pub fn observe_body(&mut self, frame: &RelayBodyFrame) -> Result<(), RelayValidationError> {
        if self.handshake != HandshakeState::Ready {
            return Err(RelayValidationError::IllegalTransition);
        }
        if frame.payload.len() > MAX_BODY_CHUNK_BYTES {
            return Err(RelayValidationError::ChunkTooLarge);
        }
        let flow = self
            .flows
            .get_mut(&frame.request_id)
            .ok_or(RelayValidationError::UnknownRequest)?;
        let (next_sequence, bytes, limit) = match (frame.kind, flow) {
            (
                RelayBodyKind::RequestChunk,
                FlowState::RequestBody {
                    next_sequence,
                    bytes,
                },
            ) => (next_sequence, bytes, MAX_REQUEST_BODY_BYTES),
            (
                RelayBodyKind::ResponseChunk,
                FlowState::ResponseBody {
                    next_sequence,
                    bytes,
                },
            ) => (next_sequence, bytes, MAX_RESPONSE_BODY_BYTES),
            _ => return Err(RelayValidationError::IllegalTransition),
        };
        if frame.sequence != *next_sequence {
            return Err(RelayValidationError::InvalidSequence);
        }
        let updated = bytes.saturating_add(frame.payload.len() as u64);
        if updated > limit {
            return Err(RelayValidationError::BodyTooLarge);
        }
        *bytes = updated;
        *next_sequence = next_sequence
            .checked_add(1)
            .ok_or(RelayValidationError::InvalidSequence)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> SecretToken {
        SecretToken::parse("c6x_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB").unwrap()
    }
    fn ready_state() -> RelaySessionState {
        let mut state = RelaySessionState::default();
        state
            .observe_control(&RelayControlFrame::ClientHello(ClientHelloFrame {
                protocol_version: 1,
                connector_token: token(),
            }))
            .unwrap();
        state
            .observe_control(&RelayControlFrame::ServerReady(ServerReadyFrame {
                installation_id: Uuid::new_v4(),
                generation: 1,
                max_concurrent_requests: 32,
                max_chunk_bytes: MAX_BODY_CHUNK_BYTES as u32,
            }))
            .unwrap();
        state
    }
    fn start(id: Uuid) -> RelayControlFrame {
        RelayControlFrame::RequestStart(RequestStartFrame {
            request_id: id,
            method: HttpMethod::new("GET").unwrap(),
            target: "/api/v1/status".into(),
            headers: vec![],
            deadline_unix_ms: 1,
        })
    }

    #[test]
    fn body_codec_round_trips_and_rejects_oversize() {
        let frame = RelayBodyFrame {
            kind: RelayBodyKind::RequestChunk,
            request_id: Uuid::new_v4(),
            sequence: 8,
            payload: vec![1, 2, 3],
        };
        assert_eq!(
            RelayBodyFrame::decode(&frame.encode().unwrap()).unwrap(),
            frame
        );
        let oversized = RelayBodyFrame {
            payload: vec![0; MAX_BODY_CHUNK_BYTES + 1],
            ..frame
        };
        assert_eq!(oversized.encode(), Err(RelayValidationError::ChunkTooLarge));
    }

    #[test]
    fn happy_path_enforces_request_then_response_order() {
        let mut state = ready_state();
        let id = Uuid::new_v4();
        state.observe_control(&start(id)).unwrap();
        state
            .observe_body(&RelayBodyFrame {
                kind: RelayBodyKind::RequestChunk,
                request_id: id,
                sequence: 0,
                payload: b"hi".to_vec(),
            })
            .unwrap();
        state
            .observe_control(&RelayControlFrame::RequestEnd(RequestIdFrame {
                request_id: id,
            }))
            .unwrap();
        state
            .observe_control(&RelayControlFrame::ResponseStart(ResponseStartFrame {
                request_id: id,
                status: 200,
                headers: vec![],
            }))
            .unwrap();
        state
            .observe_body(&RelayBodyFrame {
                kind: RelayBodyKind::ResponseChunk,
                request_id: id,
                sequence: 0,
                payload: b"ok".to_vec(),
            })
            .unwrap();
        state
            .observe_control(&RelayControlFrame::ResponseEnd(RequestIdFrame {
                request_id: id,
            }))
            .unwrap();
        assert_eq!(state.in_flight(), 0);
    }

    #[test]
    fn rejects_unknown_duplicate_and_out_of_order_frames() {
        let mut state = ready_state();
        let id = Uuid::new_v4();
        assert_eq!(
            state.observe_control(&RelayControlFrame::RequestEnd(RequestIdFrame {
                request_id: id
            })),
            Err(RelayValidationError::UnknownRequest)
        );
        state.observe_control(&start(id)).unwrap();
        assert_eq!(
            state.observe_control(&start(id)),
            Err(RelayValidationError::DuplicateRequest)
        );
        assert_eq!(
            state.observe_body(&RelayBodyFrame {
                kind: RelayBodyKind::RequestChunk,
                request_id: id,
                sequence: 2,
                payload: vec![]
            }),
            Err(RelayValidationError::InvalidSequence)
        );
        assert_eq!(
            state.observe_control(&RelayControlFrame::ResponseEnd(RequestIdFrame {
                request_id: id
            })),
            Err(RelayValidationError::IllegalTransition)
        );
    }

    #[test]
    fn enforces_concurrency_and_cumulative_body_limits() {
        let mut state = ready_state();
        for _ in 0..MAX_CONCURRENT_REQUESTS {
            state.observe_control(&start(Uuid::new_v4())).unwrap();
        }
        assert_eq!(
            state.observe_control(&start(Uuid::new_v4())),
            Err(RelayValidationError::ConcurrencyLimit)
        );

        let mut state = ready_state();
        let request_id = Uuid::new_v4();
        state.observe_control(&start(request_id)).unwrap();
        let payload = vec![0; MAX_BODY_CHUNK_BYTES];
        for sequence in 0..(MAX_REQUEST_BODY_BYTES as usize / MAX_BODY_CHUNK_BYTES) {
            state
                .observe_body(&RelayBodyFrame {
                    kind: RelayBodyKind::RequestChunk,
                    request_id,
                    sequence: sequence as u32,
                    payload: payload.clone(),
                })
                .unwrap();
        }
        assert_eq!(
            state.observe_body(&RelayBodyFrame {
                kind: RelayBodyKind::RequestChunk,
                request_id,
                sequence: (MAX_REQUEST_BODY_BYTES as usize / MAX_BODY_CHUNK_BYTES) as u32,
                payload: vec![0],
            }),
            Err(RelayValidationError::BodyTooLarge)
        );
    }

    #[test]
    fn cancellation_fences_late_chunks_and_id_reuse() {
        let mut state = ready_state();
        let request_id = Uuid::new_v4();
        state.observe_control(&start(request_id)).unwrap();
        state
            .observe_control(&RelayControlFrame::Cancel(RequestIdFrame { request_id }))
            .unwrap();
        assert_eq!(
            state.observe_body(&RelayBodyFrame {
                kind: RelayBodyKind::RequestChunk,
                request_id,
                sequence: 0,
                payload: vec![],
            }),
            Err(RelayValidationError::UnknownRequest)
        );
        assert_eq!(
            state.observe_control(&start(request_id)),
            Err(RelayValidationError::DuplicateRequest)
        );
    }

    #[test]
    fn rejects_proxy_routing_and_credential_headers() {
        for name in [
            "connection",
            "host",
            "content-length",
            "x-forwarded-for",
            "x-cresix-route",
        ] {
            let header = HeaderField {
                name: name.into(),
                value: "anything".into(),
            };
            assert_eq!(
                header.validate(),
                Err(RelayValidationError::ForbiddenHeader)
            );
        }
        assert!(
            HeaderField {
                name: "authorization".into(),
                value: "Bearer local-c6-token".into()
            }
            .validate()
            .is_ok()
        );
        assert!(
            HeaderField {
                name: "cookie".into(),
                value: "local-session=value".into()
            }
            .validate()
            .is_ok()
        );
        assert_eq!(
            HeaderField {
                name: "x-test".into(),
                value: "a\r\nb".into()
            }
            .validate(),
            Err(RelayValidationError::InvalidHeader)
        );
    }

    #[test]
    fn serde_control_frames_are_strict() {
        let valid =
            r#"{"type":"request_end","data":{"requestId":"00000000-0000-0000-0000-000000000000"}}"#;
        assert!(serde_json::from_str::<RelayControlFrame>(valid).is_ok());
        let unknown = r#"{"type":"request_end","data":{"requestId":"00000000-0000-0000-0000-000000000000","admin":true}}"#;
        assert!(serde_json::from_str::<RelayControlFrame>(unknown).is_err());
        let unknown_outer = r#"{"type":"request_end","data":{"requestId":"00000000-0000-0000-0000-000000000000"},"admin":true}"#;
        assert!(serde_json::from_str::<RelayControlFrame>(unknown_outer).is_err());
    }

    #[test]
    fn rejects_cross_surface_hello_and_network_path_targets() {
        let mut state = RelaySessionState::default();
        let bootstrap =
            SecretToken::parse("c6b_v1_AAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
        assert_eq!(
            state.observe_control(&RelayControlFrame::ClientHello(ClientHelloFrame {
                protocol_version: 1,
                connector_token: bootstrap,
            })),
            Err(RelayValidationError::IllegalTransition)
        );

        let request = RequestStartFrame {
            request_id: Uuid::new_v4(),
            method: HttpMethod::new("GET").unwrap(),
            target: "//attacker.example/path".into(),
            headers: vec![],
            deadline_unix_ms: 1,
        };
        assert_eq!(request.validate(), Err(RelayValidationError::InvalidTarget));
    }
}
