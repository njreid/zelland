/// Y-websocket compatible sync protocol implementation.
///
/// Wire format compatible with the `y-websocket` JS library:
/// - Outer message type byte: 0=sync, 1=awareness
/// - For sync messages, inner type byte: 0=SyncStep1, 1=SyncStep2, 2=Update
/// - Payloads are length-prefixed byte arrays (lib0 varUint encoding)

// Outer message types (y-websocket framing)
const MSG_SYNC: u8 = 0;
const MSG_AWARENESS: u8 = 1;

// Inner sync message types (y-protocols)
const SYNC_STEP1: u8 = 0;
const SYNC_STEP2: u8 = 1;
const SYNC_UPDATE: u8 = 2;

/// Decoded message from a y-websocket client.
#[derive(Debug)]
pub enum SyncMessage {
    /// Client sends its state vector, requesting missing updates.
    SyncStep1(Vec<u8>),
    /// Client sends an update (response to our SyncStep1).
    SyncStep2(Vec<u8>),
    /// Client sends an incremental update.
    Update(Vec<u8>),
    /// Awareness update (passed through).
    Awareness(Vec<u8>),
}

#[derive(Debug)]
pub enum SyncError {
    UnexpectedEof,
    UnknownMessageType(u64),
    UnknownSyncType(u64),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::UnexpectedEof => write!(f, "unexpected end of message"),
            SyncError::UnknownMessageType(t) => write!(f, "unknown message type: {}", t),
            SyncError::UnknownSyncType(t) => write!(f, "unknown sync type: {}", t),
        }
    }
}

impl std::error::Error for SyncError {}

/// Decode a raw WebSocket binary message.
pub fn decode_message(data: &[u8]) -> Result<SyncMessage, SyncError> {
    let (msg_type, consumed) = read_var_uint(data)?;
    let rest = &data[consumed..];

    match msg_type as u8 {
        MSG_SYNC => {
            let (sync_type, consumed2) = read_var_uint(rest)?;
            let rest2 = &rest[consumed2..];

            match sync_type as u8 {
                SYNC_STEP1 => {
                    let (payload, _) = read_var_bytes(rest2)?;
                    Ok(SyncMessage::SyncStep1(payload))
                }
                SYNC_STEP2 => {
                    let (payload, _) = read_var_bytes(rest2)?;
                    Ok(SyncMessage::SyncStep2(payload))
                }
                SYNC_UPDATE => {
                    let (payload, _) = read_var_bytes(rest2)?;
                    Ok(SyncMessage::Update(payload))
                }
                _ => Err(SyncError::UnknownSyncType(sync_type)),
            }
        }
        MSG_AWARENESS => {
            // Rest of the message is the awareness payload
            Ok(SyncMessage::Awareness(rest.to_vec()))
        }
        _ => Err(SyncError::UnknownMessageType(msg_type)),
    }
}

/// Encode a SyncStep1 message (contains our state vector).
pub fn encode_sync_step1(state_vector: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_var_uint(&mut buf, MSG_SYNC as u64);
    write_var_uint(&mut buf, SYNC_STEP1 as u64);
    write_var_bytes(&mut buf, state_vector);
    buf
}

/// Encode a SyncStep2 message (contains an update for the client).
pub fn encode_sync_step2(update: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_var_uint(&mut buf, MSG_SYNC as u64);
    write_var_uint(&mut buf, SYNC_STEP2 as u64);
    write_var_bytes(&mut buf, update);
    buf
}

/// Encode an incremental update message.
pub fn encode_update(update: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_var_uint(&mut buf, MSG_SYNC as u64);
    write_var_uint(&mut buf, SYNC_UPDATE as u64);
    write_var_bytes(&mut buf, update);
    buf
}

/// Encode an awareness message.
pub fn encode_awareness(data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_var_uint(&mut buf, MSG_AWARENESS as u64);
    buf.extend_from_slice(data);
    buf
}

// --- lib0 varUint encoding ---

/// Read a variable-length unsigned integer (lib0 format).
/// Returns (value, bytes_consumed).
fn read_var_uint(data: &[u8]) -> Result<(u64, usize), SyncError> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    for (i, &byte) in data.iter().enumerate() {
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return Err(SyncError::UnexpectedEof);
        }
    }
    Err(SyncError::UnexpectedEof)
}

/// Write a variable-length unsigned integer (lib0 format).
fn write_var_uint(buf: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

/// Read a length-prefixed byte array.
/// Returns (bytes, total_consumed).
fn read_var_bytes(data: &[u8]) -> Result<(Vec<u8>, usize), SyncError> {
    let (len, consumed) = read_var_uint(data)?;
    let len = len as usize;
    let start = consumed;
    let end = start + len;
    if end > data.len() {
        return Err(SyncError::UnexpectedEof);
    }
    Ok((data[start..end].to_vec(), end))
}

/// Write a length-prefixed byte array.
fn write_var_bytes(buf: &mut Vec<u8>, data: &[u8]) {
    write_var_uint(buf, data.len() as u64);
    buf.extend_from_slice(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_uint_roundtrip_small() {
        for value in [0u64, 1, 42, 127] {
            let mut buf = Vec::new();
            write_var_uint(&mut buf, value);
            let (decoded, consumed) = read_var_uint(&buf).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_var_uint_roundtrip_medium() {
        for value in [128u64, 255, 16383, 16384, 65535] {
            let mut buf = Vec::new();
            write_var_uint(&mut buf, value);
            let (decoded, consumed) = read_var_uint(&buf).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_var_uint_roundtrip_large() {
        let value = 1_000_000_000u64;
        let mut buf = Vec::new();
        write_var_uint(&mut buf, value);
        let (decoded, consumed) = read_var_uint(&buf).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn test_var_uint_single_byte_encoding() {
        let mut buf = Vec::new();
        write_var_uint(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        write_var_uint(&mut buf, 127);
        assert_eq!(buf, vec![0x7F]);
    }

    #[test]
    fn test_var_uint_two_byte_encoding() {
        let mut buf = Vec::new();
        write_var_uint(&mut buf, 128);
        assert_eq!(buf, vec![0x80, 0x01]);
    }

    #[test]
    fn test_var_bytes_roundtrip() {
        let data = b"hello world";
        let mut buf = Vec::new();
        write_var_bytes(&mut buf, data);
        let (decoded, consumed) = read_var_bytes(&buf).unwrap();
        assert_eq!(decoded, data);
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn test_var_bytes_empty() {
        let mut buf = Vec::new();
        write_var_bytes(&mut buf, b"");
        let (decoded, consumed) = read_var_bytes(&buf).unwrap();
        assert!(decoded.is_empty());
        assert_eq!(consumed, buf.len());
    }

    #[test]
    fn test_encode_decode_sync_step1() {
        let sv = vec![1, 2, 3, 4, 5];
        let encoded = encode_sync_step1(&sv);
        let decoded = decode_message(&encoded).unwrap();
        match decoded {
            SyncMessage::SyncStep1(data) => assert_eq!(data, sv),
            other => panic!("Expected SyncStep1, got {:?}", other),
        }
    }

    #[test]
    fn test_encode_decode_sync_step2() {
        let update = vec![10, 20, 30];
        let encoded = encode_sync_step2(&update);
        let decoded = decode_message(&encoded).unwrap();
        match decoded {
            SyncMessage::SyncStep2(data) => assert_eq!(data, update),
            other => panic!("Expected SyncStep2, got {:?}", other),
        }
    }

    #[test]
    fn test_encode_decode_update() {
        let update = vec![0xFF, 0xFE, 0xFD];
        let encoded = encode_update(&update);
        let decoded = decode_message(&encoded).unwrap();
        match decoded {
            SyncMessage::Update(data) => assert_eq!(data, update),
            other => panic!("Expected Update, got {:?}", other),
        }
    }

    #[test]
    fn test_encode_decode_awareness() {
        let awareness_data = vec![5, 6, 7, 8];
        let encoded = encode_awareness(&awareness_data);
        let decoded = decode_message(&encoded).unwrap();
        match decoded {
            SyncMessage::Awareness(data) => assert_eq!(data, awareness_data),
            other => panic!("Expected Awareness, got {:?}", other),
        }
    }

    #[test]
    fn test_decode_unknown_message_type() {
        let data = vec![99]; // Unknown type
        match decode_message(&data) {
            Err(SyncError::UnknownMessageType(99)) => {}
            other => panic!("Expected UnknownMessageType(99), got {:?}", other),
        }
    }

    #[test]
    fn test_decode_empty_message() {
        match decode_message(&[]) {
            Err(SyncError::UnexpectedEof) => {}
            other => panic!("Expected UnexpectedEof, got {:?}", other),
        }
    }

    #[test]
    fn test_decode_truncated_sync() {
        // Sync type byte but no payload
        let data = vec![0x00, 0x00]; // MSG_SYNC, SYNC_STEP1, but no var_bytes
        match decode_message(&data) {
            Err(SyncError::UnexpectedEof) => {}
            // Empty state vector is also valid (length 0)
            Ok(SyncMessage::SyncStep1(sv)) => assert!(sv.is_empty()),
            other => panic!("Expected error or empty SyncStep1, got {:?}", other),
        }
    }
}
