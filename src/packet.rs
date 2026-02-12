//! packet.rs
//! 
//! Here, we read and build packets according to the Valve RCON protocol specifications. 
//! https://developer.valvesoftware.com/wiki/Source_RCON_Protocol
//! 
//! Anatomy of a packet:
//! | Field     | Type      | Size          |
//! | --------- | --------- | ------------- |
//! | Size      | i32 (LE)  | 4 bytes       |
//! | ID        | i32 (LE)  | 4 bytes       |
//! | Type      | i32 (LE)  | 4 bytes       |
//! | Body      | String    | 1..4086 bytes |
//! | Empty Str | String    | 1 byte        |
//! | --------- | --------- | ------------- |
//! 

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{common::PacketType, errors::RconError};

#[derive(Debug)]
pub struct Packet {
    pub id: i32,
    pub packet_type: PacketType,
    pub body: String, // limited to 511 bytes w/o whitespace
}

const SIZE_FIELD_SIZE: usize = 4;
const ID_FIELD_SIZE: usize = 4;
const TYPE_FIELD_SIZE: usize = 4;
const EMPTY_PACKET_BODY_SIZE: usize = 1;
const NULL_STRING_TERMINATOR_SIZE: usize = 1;

const MINIMUM_PAYLOAD_SIZE: usize =
    ID_FIELD_SIZE + TYPE_FIELD_SIZE + EMPTY_PACKET_BODY_SIZE + NULL_STRING_TERMINATOR_SIZE;

const MAXIMUM_PACKET_SIZE: usize = 4096;
const MAXIMUM_PAYLOAD_SIZE: usize = MAXIMUM_PACKET_SIZE - SIZE_FIELD_SIZE;
const MAXIMUM_BODY_SIZE: usize = 511;

fn assert_null_terminated_body(packet: &Vec<u8>) -> Result<(), RconError> {
    let raw_body = &packet[8..];

    if raw_body.len() < 2 {
        return Err(RconError::Protocol(format!("Payload body too small")));
    }
    if raw_body[raw_body.len() - 2] != 0 {
        return Err(RconError::Protocol(format!("Body missing null terminator")));
    }
    if raw_body[raw_body.len() - 1] != 0 {
        return Err(RconError::Protocol(format!("Packet missing null terminator")));
    }

    Ok(())
}

/// Build a packet according to Valve RCON protocol definitions
/// Wiki: https://developer.valvesoftware.com/wiki/Source_RCON_Protocol
/// 
/// Key Points:
///   - Max packet size = 4096
///   - Packet Structure: [Size: 4bytes, ID: 4bytes, Type: 4bytes, Body:0..4086bytes, terminating string: 1byte]
/// 
pub fn build_packet(id: i32, kind: PacketType, body: &str) -> Result<Vec<u8>, RconError> {
    let body_bytes = body.as_bytes();
    let payload_size: usize = MINIMUM_PAYLOAD_SIZE + body_bytes.len();

    if payload_size > MAXIMUM_PAYLOAD_SIZE {
        return Err(RconError::Protocol(format!("[WRITE] payload size is too large: {}", payload_size)));
    }
    if body.len() > MAXIMUM_BODY_SIZE {
        return Err(RconError::Protocol(format!("[WRITE] packet body exceeds 511 chars: {}", body.len())))
    }

    let total_size = SIZE_FIELD_SIZE + payload_size;
    let mut buffer = Vec::with_capacity(total_size);

    buffer.extend_from_slice(&(payload_size as i32).to_le_bytes());
    buffer.extend_from_slice(&id.to_le_bytes());

    let kind_i32: i32 = kind.into();
    buffer.extend_from_slice(&kind_i32.to_le_bytes());

    buffer.extend_from_slice(body_bytes);

    buffer.push(0);
    buffer.push(0);

    assert_null_terminated_body(&buffer)?;
    return Ok(buffer);
}

/// Read a packet according to the valve docs specifications. 
pub async fn read_packet<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Packet, RconError> {
    let mut size_bytes = [0u8; 4];
    stream.read_exact(&mut size_bytes).await?;
    let size = i32::from_le_bytes(size_bytes) as usize;

    if size < MINIMUM_PAYLOAD_SIZE {
        return Err(RconError::Protocol(format!("[READ] packet size too small: {}", size)));
    } else if size > MAXIMUM_PACKET_SIZE {
        return Err(RconError::Protocol(format!("[READ] packet size too large: {}", size)));
    }

    let mut payload = vec![0u8; size];
    stream.read_exact(&mut payload).await?;

    assert_null_terminated_body(&payload)?;

    let id = i32::from_le_bytes(payload[0..4].try_into().unwrap());
    let kind_i32 = i32::from_le_bytes(payload[4..8].try_into().unwrap());
    let packet_type = PacketType::from_i32(kind_i32, false);

    let raw_body = &payload[8..];
    let end = raw_body.iter()
        .position(|&b| b == 0)
        .unwrap_or(raw_body.len());
    let body = String::from_utf8(raw_body[..end].to_vec())?;

    Ok(Packet { id, packet_type, body })
}


#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn build_packet_encodes_header_body_and_trailer() {
        // Arrange
        let id: i32 = 42;
        let packet_type = PacketType::ServerDataExecCommand;
        let body = "hi";

        // Act
        let pkt = build_packet(id, packet_type.clone(), body).unwrap();

        // Assert: compute expected bytes
        let kind_i32: i32 = packet_type.into();
        let expected_size: i32 = (MINIMUM_PAYLOAD_SIZE + body.as_bytes().len()) as i32;

        let mut expected = Vec::new();
        expected.extend_from_slice(&expected_size.to_le_bytes());
        expected.extend_from_slice(&id.to_le_bytes());
        expected.extend_from_slice(&kind_i32.to_le_bytes());
        expected.extend_from_slice(body.as_bytes());
        expected.extend_from_slice(&[0u8, 0u8]);

        assert_eq!(pkt, expected);

        // Optional extra checks (redundant but clearer failure messages)
        assert_eq!(i32::from_le_bytes(pkt[0..4].try_into().unwrap()), expected_size);
        assert_eq!(i32::from_le_bytes(pkt[4..8].try_into().unwrap()), id);
        assert_eq!(i32::from_le_bytes(pkt[8..12].try_into().unwrap()), kind_i32);
        assert_eq!(&pkt[12..pkt.len()-2], body.as_bytes());
        assert_eq!(&pkt[pkt.len()-2..], &[0, 0]);
    }

    #[test]
    fn build_packet_handles_empty_body() {
        let id = -1;
        let packet_type = PacketType::ServerDataAuth;
        let body = "";

        let pkt = build_packet(id, packet_type, body).unwrap();

        let expected_size = (MINIMUM_PAYLOAD_SIZE + 0) as i32;
        assert_eq!(i32::from_le_bytes(pkt[0..4].try_into().unwrap()), expected_size);
        assert_eq!(&pkt[pkt.len()-2..], &[0, 0]);
        assert_eq!(&pkt[12..pkt.len()-2], b"");
    }

    #[test]
    fn build_packet_rejects_body_when_packet_exceeds_max() {
        let id = 1;
        let kind = PacketType::ServerDataAuth;

        // This body makes size == MAX_PACKET_SIZE + 1
        let body_len = MAXIMUM_PAYLOAD_SIZE - MINIMUM_PAYLOAD_SIZE + 1;
        let body = "a".repeat(body_len);

        let result = build_packet(id, kind, &body);

        assert!(matches!(result, Err(RconError::Protocol(_))));
    }

    #[tokio::test]
    async fn read_packet_parses_valid_packet_from_stream() {
        let id = 123;
        let packet_type = PacketType::ServerDataAuth;
        let body = "hello";

        let bytes = build_packet(id, packet_type.clone(), body).unwrap();
        let mut cur = Cursor::new(bytes);

        let pkt = read_packet(&mut cur).await.unwrap();

        assert_eq!(pkt.id, id);
        assert_eq!(pkt.packet_type, packet_type);
        assert_eq!(pkt.body, body);
    }

    #[tokio::test]
    async fn read_packet_rejects_declared_size_over_max() {
        let declared_size = (MAXIMUM_PACKET_SIZE as i32) + 1;
        let id: i32 = 1;
        let type_i32: i32 = PacketType::ServerDataAuth.into();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&declared_size.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&type_i32.to_le_bytes());
        bytes.extend_from_slice(&[0,0]);
        bytes.extend_from_slice(&[0,0]);

        let mut cur = Cursor::new(bytes);
        let err = read_packet(&mut cur).await.unwrap_err();
        
        assert!(matches!(err, RconError::Protocol(_)));
    }

    #[tokio::test]
    async fn read_packet_rejects_bad_terminator() {
        let mut bytes = build_packet(
            1, 
            PacketType::ServerDataAuth, 
            "hello"
        ).unwrap();

        let n = bytes.len();
        bytes[n - 2] = 1;
        bytes[n - 1] = 2;

        let mut cur = Cursor::new(bytes);
        let err = read_packet(&mut cur).await.unwrap_err();

        assert!(matches!(err, RconError::Protocol(_)));
    }

}