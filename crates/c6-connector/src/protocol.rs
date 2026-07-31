//! Relay wire contracts are owned by `c6-cloud-core`. Re-exporting them here
//! keeps the connector from creating a subtly incompatible second schema.

pub use c6_cloud_core::{
    ClientHelloFrame, HeaderField, HttpMethod, KeepaliveFrame, MAX_BODY_CHUNK_BYTES,
    MAX_REQUEST_BODY_BYTES, MAX_RESPONSE_BODY_BYTES, RELAY_PROTOCOL_VERSION, RELAY_SUBPROTOCOL,
    RelayBodyFrame, RelayBodyKind, RelayControlFrame, RelayFailureCode, RelaySessionState,
    RelayValidationError, RequestFailedFrame, RequestIdFrame, RequestStartFrame,
    ResponseStartFrame, ServerReadyFrame,
};

pub fn decode_control(text: &str) -> Result<RelayControlFrame, RelayValidationError> {
    if text.len() > 64 * 1024 {
        return Err(RelayValidationError::HeadersTooLarge);
    }
    serde_json::from_str(text).map_err(|_| RelayValidationError::IllegalTransition)
}

pub fn encode_control(frame: &RelayControlFrame) -> Result<String, RelayValidationError> {
    serde_json::to_string(frame).map_err(|_| RelayValidationError::IllegalTransition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn connector_uses_strict_shared_control_contract() {
        let frame = RelayControlFrame::Ping(KeepaliveFrame { nonce: 9 });
        assert_eq!(
            decode_control(&encode_control(&frame).unwrap()).unwrap(),
            frame
        );
        assert!(decode_control(r#"{"type":"ping","data":{"nonce":9,"extra":true}}"#).is_err());
    }

    #[test]
    fn connector_uses_sequenced_shared_binary_codec() {
        let frame = RelayBodyFrame {
            kind: RelayBodyKind::RequestChunk,
            request_id: Uuid::new_v4(),
            sequence: 3,
            payload: vec![0, 1, 255],
        };
        assert_eq!(
            RelayBodyFrame::decode(&frame.encode().unwrap()).unwrap(),
            frame
        );
    }
}
