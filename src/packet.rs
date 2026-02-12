use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{common::PacketType, errors::RconError};

#[derive(Debug)]
pub struct Packet {
    pub id: i32,
    pub packet_type: PacketType,
    pub body: String, // limited to 511 bytes w/o whitespace
}

const SIZE_PAYLOAD_SIZE: usize = 4;
const ID_PAYLOAD_SIZE: usize = 4;
const EMPTY_PACKET_BODY_SIZE: usize = 1;
const NULL_STRING_TERMINATOR_SIZE: usize = 1;

const MINIMUM_PAYLOAD_SIZE: usize =
    SIZE_PAYLOAD_SIZE + ID_PAYLOAD_SIZE + EMPTY_PACKET_BODY_SIZE + NULL_STRING_TERMINATOR_SIZE;

pub fn build_packet(id: i32, kind: PacketType, body: &str) -> Vec<u8> {
    let body_bytes = body.as_bytes();
    let size: usize = MINIMUM_PAYLOAD_SIZE + body_bytes.len();

    let mut buffer = Vec::with_capacity(size);

    buffer.extend_from_slice(&(size as i32).to_le_bytes());
    buffer.extend_from_slice(&id.to_le_bytes());

    let kind_i32: i32 = kind.into();
    buffer.extend_from_slice(&kind_i32.to_le_bytes());

    buffer.extend_from_slice(body_bytes);

    buffer.push(0);
    buffer.push(0);
    return buffer;
}

pub async fn read_packet<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Packet, RconError> {
    let mut size_bytes = [0u8; 4];
    stream.read_exact(&mut size_bytes).await?;
    let size = i32::from_le_bytes(size_bytes) as usize;

    if size < MINIMUM_PAYLOAD_SIZE {
        return Err(RconError::Protocol("packet size too small".to_string()));
    }

    let mut payload = vec![0u8; size];
    stream.read_exact(&mut payload).await?;

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
