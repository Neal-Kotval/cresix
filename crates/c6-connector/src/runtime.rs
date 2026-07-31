use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use http::{
    HeaderValue, Request,
    header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL},
};
use rand::Rng;
use thiserror::Error;
use tokio::{sync::mpsc, time::sleep};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Message, client::IntoClientRequest},
};
use uuid::Uuid;

use crate::{
    LoadedConfig,
    protocol::{
        ClientHelloFrame, KeepaliveFrame, MAX_BODY_CHUNK_BYTES, RELAY_PROTOCOL_VERSION,
        RELAY_SUBPROTOCOL, RelayBodyFrame, RelayBodyKind, RelayControlFrame, RelayFailureCode,
        RelaySessionState, RequestFailedFrame, RequestIdFrame, ResponseStartFrame, decode_control,
        encode_control,
    },
    proxy::{FixedUpstream, ProxyError, ProxyRequest, ProxyResponse},
};

const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("connector authentication was rejected; rotate or replace its credential")]
    AuthenticationRejected,
    #[error("relay protocol violation")]
    Protocol,
    #[error("relay transport failed")]
    Transport,
    #[error("connector configuration could not construct a relay request")]
    Configuration,
}

struct PendingRequest {
    method: String,
    target: String,
    headers: Vec<crate::protocol::HeaderField>,
    body: Vec<u8>,
    deadline_unix_ms: u64,
    rejected: bool,
}

enum Outgoing {
    Response(Uuid, Result<ProxyResponse, ProxyError>),
}

/// Runs until cancelled or an authentication failure requires operator action.
pub async fn run_reconnecting(config: Arc<LoadedConfig>) -> Result<(), RuntimeError> {
    let mut delay = Duration::from_secs(1);
    loop {
        match run_session(config.clone()).await {
            Ok(()) | Err(RuntimeError::Transport) => {
                let upper = delay.as_millis().max(1) as u64;
                let jitter = rand::rng().random_range(0..=upper / 4);
                sleep(delay + Duration::from_millis(jitter)).await;
                delay = (delay * 2).min(MAX_RECONNECT_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

async fn run_session(config: Arc<LoadedConfig>) -> Result<(), RuntimeError> {
    let request = websocket_request(&config)?;
    let (stream, response) = connect_async(request).await.map_err(classify_ws)?;
    if response
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        != Some(RELAY_SUBPROTOCOL)
    {
        return Err(RuntimeError::Protocol);
    }
    let (mut sink, mut source) = stream.split();
    let mut state = RelaySessionState::default();
    send_observed_control(
        &mut sink,
        &mut state,
        &RelayControlFrame::ClientHello(ClientHelloFrame {
            protocol_version: RELAY_PROTOCOL_VERSION,
            connector_token: config.credentials.cloud().clone(),
        }),
    )
    .await?;

    let ready = source
        .next()
        .await
        .ok_or(RuntimeError::Transport)?
        .map_err(|_| RuntimeError::Transport)?;
    let Message::Text(ready) = ready else {
        return Err(RuntimeError::Protocol);
    };
    let ready = decode_control(&ready).map_err(|_| RuntimeError::Protocol)?;
    state
        .observe_control(&ready)
        .map_err(|_| RuntimeError::Protocol)?;
    let RelayControlFrame::ServerReady(ready) = ready else {
        return Err(RuntimeError::Protocol);
    };
    if ready.installation_id != config.config.installation_id {
        return Err(RuntimeError::Protocol);
    }

    let upstream = FixedUpstream::new(config.local_origin.clone(), config.request_timeout())
        .map_err(|_| RuntimeError::Configuration)?;
    let mut pending = HashMap::<Uuid, PendingRequest>::new();
    let mut active = HashSet::<Uuid>::new();
    let mut cancelled = HashSet::<Uuid>::new();
    let (out_tx, mut out_rx) = mpsc::channel::<Outgoing>(config.config.max_in_flight);

    loop {
        tokio::select! {
            incoming = source.next() => {
                let Some(incoming) = incoming else { return Err(RuntimeError::Transport); };
                let incoming = incoming.map_err(|_| RuntimeError::Transport)?;
                match incoming {
                    Message::Text(text) => {
                        let frame = decode_control(&text).map_err(|_| RuntimeError::Protocol)?;
                        state.observe_control(&frame).map_err(|_| RuntimeError::Protocol)?;
                        match frame {
                            RelayControlFrame::RequestStart(frame) => {
                                let request_id = frame.request_id;
                                if pending.contains_key(&request_id) {
                                    return Err(RuntimeError::Protocol);
                                }
                                let rejected = pending.len().saturating_add(active.len()) >= config.config.max_in_flight;
                                pending.insert(request_id, PendingRequest {
                                    method: frame.method.as_str().to_owned(), target: frame.target,
                                    headers: frame.headers, body: Vec::new(),
                                    deadline_unix_ms: frame.deadline_unix_ms, rejected,
                                });
                            }
                            RelayControlFrame::RequestEnd(frame) => {
                                let request_id = frame.request_id;
                                if cancelled.remove(&request_id) { continue; }
                                let Some(request) = pending.remove(&request_id) else { return Err(RuntimeError::Protocol); };
                                if request.rejected {
                                    send_failure(&mut sink, &mut state, request_id, RelayFailureCode::BadGateway, "connector concurrency limit reached").await?;
                                    continue;
                                }
                                let now = unix_millis();
                                if request.deadline_unix_ms <= now {
                                    send_failure(&mut sink, &mut state, request_id, RelayFailureCode::Timeout, "request deadline expired").await?;
                                    continue;
                                }
                                let deadline = Duration::from_millis(request.deadline_unix_ms - now)
                                    .min(config.request_timeout());
                                let upstream = upstream.clone();
                                let tx = out_tx.clone();
                                active.insert(request_id);
                                tokio::spawn(async move {
                                    let response = match tokio::time::timeout(deadline, upstream.execute(ProxyRequest {
                                            method: request.method,
                                            target: request.target,
                                            headers: request.headers,
                                            body: request.body,
                                        })).await {
                                        Ok(response) => response,
                                        Err(_) => Err(ProxyError::Timeout),
                                    };
                                    let _ = tx.send(Outgoing::Response(request_id, response)).await;
                                });
                            }
                            RelayControlFrame::Cancel(frame) => {
                                if pending.remove(&frame.request_id).is_none() && active.contains(&frame.request_id) {
                                    cancelled.insert(frame.request_id);
                                }
                            }
                            RelayControlFrame::Ping(frame) => send_observed_control(&mut sink, &mut state, &RelayControlFrame::Pong(KeepaliveFrame { nonce: frame.nonce })).await?,
                            RelayControlFrame::Pong(_) => {}
                            _ => return Err(RuntimeError::Protocol),
                        }
                    }
                    Message::Binary(bytes) => {
                        let frame = RelayBodyFrame::decode(&bytes).map_err(|_| RuntimeError::Protocol)?;
                        state.observe_body(&frame).map_err(|_| RuntimeError::Protocol)?;
                        let RelayBodyFrame { kind: RelayBodyKind::RequestChunk, request_id, payload: bytes, .. } = frame else {
                            return Err(RuntimeError::Protocol);
                        };
                        let Some(request) = pending.get_mut(&request_id) else { return Err(RuntimeError::Protocol); };
                        request.body.extend_from_slice(&bytes);
                    }
                    Message::Ping(bytes) => sink.send(Message::Pong(bytes)).await.map_err(|_| RuntimeError::Transport)?,
                    Message::Pong(_) => {}
                    Message::Close(_) => return Ok(()),
                    _ => return Err(RuntimeError::Protocol),
                }
            }
            outgoing = out_rx.recv() => {
                let Some(Outgoing::Response(id, response)) = outgoing else { return Err(RuntimeError::Transport); };
                active.remove(&id);
                if cancelled.remove(&id) { continue; }
                match response {
                    Ok(response) => send_response(&mut sink, &mut state, id, response).await?,
                    Err(error) => send_failure(&mut sink, &mut state, id, failure_code(error), "local C6 request failed").await?,
                }
            }
        }
    }
}

fn websocket_request(config: &LoadedConfig) -> Result<Request<()>, RuntimeError> {
    let mut url = config.cloud_origin.clone();
    url.set_scheme(if url.scheme() == "https" { "wss" } else { "ws" })
        .map_err(|_| RuntimeError::Configuration)?;
    url.set_path("/api/v1/relay/connect");
    let authorization = HeaderValue::from_str(&format!(
        "Bearer {}",
        config.credentials.cloud().expose_secret()
    ))
    .map_err(|_| RuntimeError::Configuration)?;
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| RuntimeError::Configuration)?;
    request.headers_mut().insert(AUTHORIZATION, authorization);
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(RELAY_SUBPROTOCOL),
    );
    Ok(request)
}

async fn send_observed_control<S>(
    sink: &mut S,
    state: &mut RelaySessionState,
    frame: &RelayControlFrame,
) -> Result<(), RuntimeError>
where
    S: futures_util::Sink<Message> + Unpin,
{
    state
        .observe_control(frame)
        .map_err(|_| RuntimeError::Protocol)?;
    sink.send(Message::Text(
        encode_control(frame)
            .map_err(|_| RuntimeError::Protocol)?
            .into(),
    ))
    .await
    .map_err(|_| RuntimeError::Transport)
}

async fn send_failure<S>(
    sink: &mut S,
    state: &mut RelaySessionState,
    request_id: Uuid,
    code: RelayFailureCode,
    message: &str,
) -> Result<(), RuntimeError>
where
    S: futures_util::Sink<Message> + Unpin,
{
    send_observed_control(
        sink,
        state,
        &RelayControlFrame::RequestFailed(RequestFailedFrame {
            request_id,
            code,
            message: message.to_owned(),
        }),
    )
    .await
}

async fn send_response<S>(
    sink: &mut S,
    state: &mut RelaySessionState,
    request_id: Uuid,
    response: ProxyResponse,
) -> Result<(), RuntimeError>
where
    S: futures_util::Sink<Message> + Unpin,
{
    send_observed_control(
        sink,
        state,
        &RelayControlFrame::ResponseStart(ResponseStartFrame {
            request_id,
            status: response.status,
            headers: response.headers,
        }),
    )
    .await?;
    for (sequence, chunk) in response.body.chunks(MAX_BODY_CHUNK_BYTES).enumerate() {
        let frame = RelayBodyFrame {
            kind: RelayBodyKind::ResponseChunk,
            request_id,
            sequence: sequence as u32,
            payload: chunk.to_vec(),
        };
        state
            .observe_body(&frame)
            .map_err(|_| RuntimeError::Protocol)?;
        sink.send(Message::Binary(
            frame.encode().map_err(|_| RuntimeError::Protocol)?.into(),
        ))
        .await
        .map_err(|_| RuntimeError::Transport)?;
    }
    send_observed_control(
        sink,
        state,
        &RelayControlFrame::ResponseEnd(RequestIdFrame { request_id }),
    )
    .await
}

fn failure_code(error: ProxyError) -> RelayFailureCode {
    match error {
        ProxyError::InvalidRequest => RelayFailureCode::Protocol,
        ProxyError::Unavailable | ProxyError::ResponseTooLarge => RelayFailureCode::BadGateway,
        ProxyError::Timeout => RelayFailureCode::Timeout,
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn classify_ws(error: tungstenite::Error) -> RuntimeError {
    if let tungstenite::Error::Http(response) = &error
        && matches!(response.status().as_u16(), 401 | 403)
    {
        return RuntimeError::AuthenticationRejected;
    }
    RuntimeError::Transport
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_errors_have_stable_non_sensitive_codes() {
        assert_eq!(
            failure_code(ProxyError::InvalidRequest),
            RelayFailureCode::Protocol
        );
        assert_eq!(
            failure_code(ProxyError::Unavailable),
            RelayFailureCode::BadGateway
        );
        assert_eq!(failure_code(ProxyError::Timeout), RelayFailureCode::Timeout);
        assert_eq!(
            failure_code(ProxyError::ResponseTooLarge),
            RelayFailureCode::BadGateway
        );
    }

    #[test]
    fn reconnect_delay_is_bounded() {
        let mut delay = Duration::from_secs(1);
        for _ in 0..100 {
            delay = (delay * 2).min(MAX_RECONNECT_DELAY);
        }
        assert_eq!(delay, MAX_RECONNECT_DELAY);
    }
}
