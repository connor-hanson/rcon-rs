use tokio::{
    io::{
        AsyncRead, 
        AsyncWrite,
        AsyncWriteExt,
    }, 
    time::timeout
};

use crate::packet::{
    Packet,
    read_packet, 
    build_packet
};
use crate::{
    client::RconClient, 
    common::PacketType, 
    errors::RconError
};

impl<S: AsyncRead + AsyncWrite + Unpin> RconClient<S> {
    /// Allocates a new packet id for the next packet to be sent.
    /// This is the expected return value of the next call to `write_packet`, and is used to match responses to requests.
    fn alloc_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id = if self.next_id >= i32::MAX - 10 { 1 } else { self.next_id + 1 };
        return id;
    }

    /// Writes a packet to the client stream with the given type and body.
    /// The write is waited on for at most `self.io_timeout` duration, after which a `RconError::Timeout` is returned.
    /// 
    /// ### Parameters
    /// - packet_type The type of packet to write
    /// - body The body of the packet to write
    /// 
    /// ### Returns
    /// - The id of the packet that was written, or an error if the write failed or timed out.
    pub(crate) async fn write_packet(&mut self, packet_type: PacketType, body: &str) -> Result<i32, RconError> {
        let id = self.alloc_id();
        let buf = build_packet(id, packet_type.clone(), body)?;
        timeout(self.client_config.io_timeout, self.stream.write_all(&buf))
            .await
            .map_err(|_| RconError::Timeout)??;
        log::debug!("Sent {:?} packet with id: {:?}", packet_type, id);
        Ok(id)
    }

    pub(crate) async fn read_packet(&mut self) -> Result<Packet, RconError> {
        log::debug!("Waiting for packet...");
        let res = timeout(self.client_config.io_timeout, read_packet(&mut self.stream))
            .await
            .map_err(|_| RconError::Timeout)?;

        log::debug!("Received packet: {:?}", res);
        return res;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::RconClientConfig;

    use super::*;

    use tokio::io::{
        AsyncWriteExt,
        AsyncReadExt,
        duplex,
    };

    const MAX_BUF_SIZE: usize = 16384;

    #[tokio::test]
    async fn write_packet_writes_expected_bytes() {
        let (client_stream, mut server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream);

        let ptype = PacketType::ServerDataAuth;
        let pw = "pw";

        let id = client.write_packet(ptype.clone(), pw).await.unwrap();
        let expected = build_packet(id, ptype, pw).unwrap();

        let mut received: Vec<u8> = vec![0u8; expected.len()];
        server_stream.read_exact(&mut received).await.unwrap();

        assert_eq!(expected, received);
    }

    #[tokio::test]
    async fn write_packet_writes_empty_packet() {
        let (client_stream, mut server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream);

        let ptype = PacketType::ServerDataAuth;
        let pw = "";

        let id = client.write_packet(ptype.clone(), pw).await.unwrap();
        let expected = build_packet(id, ptype, pw).unwrap();

        let mut received: Vec<u8> = vec![0u8; expected.len()];
        server_stream.read_exact(&mut received).await.unwrap();

        assert_eq!(expected, received);
    }

    #[tokio::test]
    async fn read_packet_reads_packet_from_stream() {
        let (client_stream, mut server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream);

        let bytes = build_packet(123, PacketType::ServerDataAuthResponse, "ok").unwrap();
        server_stream.write_all(&bytes).await.unwrap();

        let packet = client.read_packet().await.unwrap();

        // Hack
        let pkt_type_i32: i32 = packet.packet_type.into();
        let expected_type: i32 = PacketType::ServerDataAuthResponse.into();

        assert_eq!(packet.id, 123);
        assert_eq!(pkt_type_i32, expected_type);
        assert_eq!(packet.body, "ok");
    }

    #[tokio::test]
    async fn read_packet_times_out_when_no_packet_is_received() {
        let (client_stream, _server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream)
            .with_client_config(RconClientConfig {
                io_timeout: Duration::from_millis(1),
                ..Default::default()
            });

        let res = client.read_packet().await;
        assert!(matches!(res, Err(RconError::Timeout)));
    }

    #[tokio::test]
    async fn write_packet_times_out_when_write_is_not_completed() {
        // Key trick: make duplex capacity tiny and write a lot so write_all blocks.
        let (client_stream, _server_stream) = duplex(8); // tiy buf, peer never reads
        let mut client = RconClient::new(client_stream)
            .with_client_config(RconClientConfig {
                io_timeout: Duration::from_millis(1),
                ..Default::default()
            });

        // build a body that's allowed by your 511 char limit but large enough to fill duplex
        let body = "a".repeat(200); // tune if needed
        let err = client
            .write_packet(PacketType::ServerDataAuth, &body)
            .await
            .unwrap_err();

        assert!(matches!(err, RconError::Timeout));
    }

    #[tokio::test]
    async fn alloc_id_wraps_near_i32_max() {
        let (duplex_client, _) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(duplex_client);

        client.next_id = i32::MAX - 10;
        let a = client.alloc_id();
        let b = client.alloc_id();

        assert_eq!(a, i32::MAX - 10);
        assert_eq!(b, 1);
    }
}