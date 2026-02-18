use std::{io, time::Duration};
use tokio::{
    net::TcpStream, 
    io::{
        AsyncRead, 
        AsyncWrite, 
        AsyncWriteExt
    }, 
    time::timeout
};
use log;

use crate::{common::PacketType, errors::RconError, packet::{Packet, build_packet, read_packet}};

const IDLE_TIMEOUT_MILLIS: u64 = 150;

#[derive(Debug, Clone, Copy)]
pub struct RconClient<S> {
    stream: S,
    next_id: i32,
    io_timeout: Duration,
}

impl RconClient<TcpStream> {
    pub async fn connect(addr: impl AsRef<str>) -> io::Result<Self> {
        let stream = TcpStream::connect(addr.as_ref()).await?;
        return Ok(Self::new(stream));
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> RconClient<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            next_id: 1,
            io_timeout: Duration::from_secs(5)
        }
    }

    fn alloc_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id = if self.next_id >= i32::MAX - 10 { 1 } else { self.next_id + 1 };
        return id
    }

    async fn write_packet(&mut self, id: i32, packet_type: PacketType, body: &str) -> Result<(), RconError> {
        let buf = build_packet(id, packet_type.clone(), body)?;
        timeout(self.io_timeout, self.stream.write_all(&buf))
            .await
            .map_err(|_| RconError::Timeout)??;
        log::debug!("Sent {:?} packet with id: {:?}", packet_type, id);
        Ok(())
    }

    async fn read_packet_timed(&mut self) -> Result<Packet, RconError> {
        log::debug!("Waiting for packet...");
        let res = timeout(self.io_timeout, read_packet(&mut self.stream))
            .await
            .map_err(|_| RconError::Timeout)?;

        log::debug!("Received packet: {:?}", res);
        return res;
    }

    /// Authenticate using password.
    /// Many servers will send multiple packets. We read until AUTH_RESPONSE.
    pub async fn auth(&mut self, password: &str) -> Result<(), RconError> {
        log::debug!("Starting authentication...");

        let id = self.alloc_id();
        self.write_packet(id, PacketType::ServerDataAuth, password).await?;

        loop {
            let packet: Packet = self.read_packet_timed().await?;

            // Servers may send a response value first
            // ignore it and keep reading
            if packet.packet_type == PacketType::ServerDataAuthResponse || 
               packet.packet_type == PacketType::ServerDataExecCommand 
            {
                if packet.id == -1 {
                    return Err(RconError::AuthFailed);
                }
                if packet.id != id {
                    return Err(RconError::Protocol("Auth packet ID response mismatch".to_string()));
                }
                return Ok(());
            }
        }
    }

    /// Execute a command and return the combined response. 
    /// 
    /// Source RCON may split responses across multiple RESPONSE_VALUE packets.
    /// A short timeout is implemented to wait for packets to finish sending
    pub async fn exec(&mut self, command: &str) -> Result<String, RconError> {
        log::debug!("Received command: {}", command);

        let cmd_id = self.alloc_id();
        self.write_packet(cmd_id, PacketType::ServerDataExecCommand, command).await?;

        let mut out = String::new();

        let idle = Duration::from_millis(IDLE_TIMEOUT_MILLIS);
        let mut data_seen = false;

        loop {
            match timeout(idle, self.read_packet_timed()).await {
                Ok(Ok(pkt)) => {
                    if pkt.id != cmd_id {
                        continue;
                    }

                    data_seen = true;
                    let ptype: i32 = pkt.packet_type.into();
                    match ptype {
                        0 => out.push_str(&pkt.body),
                        2 => out.push_str(&pkt.body),
                        _ => ()
                    }
                },
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    if data_seen {
                        break;
                    }
                }
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::i32;

    use super::*;

    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    const MAX_TEST_BUF_SIZE: usize = 4096;

    // Helper: make a client with a short io_timeout so timeout tests run fast
    fn client_with_timeout<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
        stream: S,
        ms: u64,
    ) -> RconClient<S> {
        let mut c = RconClient::new(stream);
        c.io_timeout = Duration::from_millis(ms);
        c
    }

    #[tokio::test]
    async fn write_packet_writes_expected_bytes() {
        let (client_side, mut server_side) = duplex(16_384);
        let mut client = client_with_timeout(client_side, 200);

        let id = 7;
        let ptype = PacketType::ServerDataAuth;
        let body = "pw";

        // Act: write from client
        client.write_packet(id, ptype.clone(), body).await.unwrap();

        // Assert: server reads exactly the bytes build_packet would produce
        let expected = build_packet(id, ptype, body).unwrap();

        let mut got = vec![0u8; expected.len()];
        server_side.read_exact(&mut got).await.unwrap();

        assert_eq!(got, expected);
    }

    #[tokio::test]
    async fn read_packet_timed_reads_packet_from_stream() {
        let (client_side, mut server_side) = duplex(16_384);
        let mut client = client_with_timeout(client_side, 200);

        // Server side sends a packet
        let bytes = build_packet(123, PacketType::ServerDataAuthResponse, "ok").unwrap();
        server_side.write_all(&bytes).await.unwrap();

        // Client reads it
        let pkt = client.read_packet_timed().await.unwrap();

        let pkt_type_i32: i32 = pkt.packet_type.into();
        let expected_type: i32 = PacketType::ServerDataAuthResponse.into();

        assert_eq!(pkt.id, 123);
        assert_eq!(pkt_type_i32, expected_type);
        assert_eq!(pkt.body, "ok");
    }

    #[tokio::test]
    async fn read_packet_timed_times_out_when_no_data_arrives() {
        let (client_side, _server_side) = duplex(16_384);
        let mut client = client_with_timeout(client_side, 50);

        let err = client.read_packet_timed().await.unwrap_err();
        assert!(matches!(err, RconError::Timeout));
    }

    #[tokio::test]
    async fn write_packet_times_out_when_peer_never_reads_and_buffer_fills() {
        // Key trick: make duplex capacity tiny and write a lot so write_all blocks.
        let (client_side, _server_side) = duplex(8); // tiny buffer; peer never reads
        let mut client = client_with_timeout(client_side, 50);

        // build a body that's allowed by your 511 char limit but large enough to fill duplex
        let body = "a".repeat(200); // tune if needed
        let err = client
            .write_packet(1, PacketType::ServerDataAuth, &body)
            .await
            .unwrap_err();

        assert!(matches!(err, RconError::Timeout));
    }

    #[tokio::test]
    async fn alloc_id_wraps_near_i32_max() {
        let (duplex_client, _) = duplex(MAX_TEST_BUF_SIZE);
        let mut client = RconClient::new(duplex_client);

        client.next_id = i32::MAX - 10;
        let a = client.alloc_id();
        let b = client.alloc_id();

        assert_eq!(a, i32::MAX - 10);
        assert_eq!(b, 1);
    }

    #[tokio::test]
    async fn auth_succeeds_on_matching_auth_response() {
        let (duplex_client, mut server) = duplex(MAX_TEST_BUF_SIZE);
        let mut client = RconClient::new(duplex_client);

        let server = tokio::spawn(async move {
            let req = read_packet(&mut server).await.unwrap();
            assert_eq!(req.packet_type, PacketType::ServerDataAuth);

            let resp = build_packet(req.id, PacketType::ServerDataAuthResponse, "").unwrap();
            server.write_all(&resp).await.unwrap();
        });

        client.auth("pw").await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn auth_fails_when_server_returns_minus_one_id() {
                let (duplex_client, mut server) = duplex(MAX_TEST_BUF_SIZE);
        let mut client = RconClient::new(duplex_client);

        let server = tokio::spawn(async move {
            let _req = read_packet(&mut server).await.unwrap();

            let resp = build_packet(-1, PacketType::ServerDataAuthResponse, "").unwrap();
            server.write_all(&resp).await.unwrap();
        });

        let err= client.auth("pw").await.unwrap_err();
        assert!(matches!(err, RconError::AuthFailed));

        server.await.unwrap();
    }

    #[tokio::test]
    async fn auth_ignores_unrelated_packets_first() {
                let (duplex_client, mut server) = duplex(MAX_TEST_BUF_SIZE);
        let mut client = RconClient::new(duplex_client);

        let server = tokio::spawn(async move {
            let req = read_packet(&mut server).await.unwrap();
            assert_eq!(req.packet_type, PacketType::ServerDataAuth);

            let junk = build_packet(req.id, PacketType::ServerDataResponseValue, "junk").unwrap();
            server.write_all(&junk).await.unwrap();

            let resp = build_packet(req.id, PacketType::ServerDataAuthResponse, "").unwrap();
            server.write_all(&resp).await.unwrap();
        });

        client.auth("pw").await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn exec_aggregates_multiple_packets_then_stops_on_idle() {
        let (client_side, mut server_side) = duplex(16_384);
        let mut client = RconClient::new(client_side);

        // Fake server: read the command packet, then send two response chunks with same id.
        let server = tokio::spawn(async move {
            // Read the outgoing command packet
            let cmd = read_packet(&mut server_side).await.unwrap();
            assert_eq!(cmd.packet_type, PacketType::ServerDataExecCommand);
            assert_eq!(cmd.body, "status");

            // Send responses with the SAME id as cmd
            let r1 = build_packet(cmd.id, PacketType::ServerDataExecCommand, "hello ").unwrap();
            server_side.write_all(&r1).await.unwrap();

            let r2 = build_packet(cmd.id, PacketType::ServerDataExecCommand, "world").unwrap();
            server_side.write_all(&r2).await.unwrap();

            // Then do nothing: client should exit on idle timeout once data_seen is true.
            // Important: keep the task alive a bit so the client can read.
            tokio::time::sleep(Duration::from_millis(2 * IDLE_TIMEOUT_MILLIS as u64)).await;
        });

        let out = client.exec("status").await.unwrap();
        assert_eq!(out, "hello world");

        server.await.unwrap();
    }

    #[tokio::test]
    async fn exec_ignores_unrelated_packet_ids() {
        let (client_side, mut server_side) = duplex(16_384);
        let mut client = RconClient::new(client_side);

        let server = tokio::spawn(async move {
            let cmd = read_packet(&mut server_side).await.unwrap();
            assert_eq!(cmd.packet_type, PacketType::ServerDataExecCommand);

            // Unrelated packet (different id) should be ignored by client
            let unrelated = build_packet(cmd.id + 9999, PacketType::ServerDataExecCommand, "NOPE").unwrap();
            server_side.write_all(&unrelated).await.unwrap();

            // Related packet should be appended
            let good = build_packet(cmd.id, PacketType::ServerDataExecCommand, "OK").unwrap();
            server_side.write_all(&good).await.unwrap();

            // Let client idle out
            tokio::time::sleep(Duration::from_millis(2 * IDLE_TIMEOUT_MILLIS as u64)).await;
        });

        let out = client.exec("anything").await.unwrap();
        assert_eq!(out, "OK");

        server.await.unwrap();
    }

}