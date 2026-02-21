use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::client_config;
use crate::{client::RconClient, errors::RconError, packet::Packet, common::PacketType};

impl RconClient<TcpStream> {
    pub async fn connect(
        client_config: client_config::RconClientConfig
    ) -> Result<Self, RconError> {
        let stream = TcpStream::connect((client_config.address.as_str(), client_config.port)).await?;
        let mut client = RconClient::new(stream).with_client_config(client_config);
        client.authenticate().await?;

        Ok(client)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> RconClient<S> {
    pub async fn authenticate(&mut self) -> Result<(), RconError> {
        log::debug!("Starting authentication...");
        let expected_id = self.write_packet(PacketType::ServerDataAuth, &self.client_config.password.clone()).await?;

        loop {
            let pkt: Packet = self.read_packet().await?;
            if pkt.packet_type != PacketType::ServerDataAuthResponse && pkt.packet_type != PacketType::ServerDataExecCommand {
                log::debug!("Received non-auth response packet while waiting for auth response, ignoring: {:?}", pkt);
                continue;
            }

            if pkt.id == -1 {
                return Err(RconError::AuthFailed);
            }
            if pkt.id != expected_id {
                return Err(
                    RconError::Protocol(format!("Auth packet id response mismatch. Expected: {:?}, got: {:?}", expected_id, pkt.id))
                )
            }

            return Ok(());
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::RconClientConfig;

    use super::*;
    use tokio::io::duplex;

    const MAX_BUF_SIZE: usize = 4096;
    
    #[tokio::test]
    async fn auth_succeeds_on_matching_auth_response() {
        let (client_stream, server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream)
            .with_client_config(RconClientConfig {
                password: "pw".to_string(),
                ..Default::default()
            });

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream);

            let pkt = server_client.read_packet().await.unwrap();
            server_client = server_client.with_next_id(pkt.id);

            assert_eq!(pkt.packet_type, PacketType::ServerDataAuth);
            assert_eq!(pkt.body, "pw");

            server_client.write_packet(PacketType::ServerDataAuthResponse, "").await.unwrap();
        });

        client.authenticate().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn auth_fails_on_failed_auth_response() {
        let (client_stream, server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream)
            .with_client_config(RconClientConfig {
                password: "pw".to_string(),
                ..Default::default()
            });

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream)
                .with_next_id(-1); // -1 is the id used by the server to indicate failed auth
            let _req = server_client.read_packet().await.unwrap();
            server_client.write_packet(PacketType::ServerDataAuthResponse, "").await.unwrap();
        });

        let auth_result = client.authenticate().await;
        assert!(auth_result.is_err());
        assert!(matches!(auth_result.err().unwrap(), RconError::AuthFailed));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn auth_ignores_unrelated_packets() {
        let (client_stream, server_stream) = duplex(MAX_BUF_SIZE);
        let mut client = RconClient::new(client_stream);

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream);
            let req = server_client.read_packet().await.unwrap();
            server_client = server_client.with_next_id(req.id);
            server_client.write_packet(PacketType::ServerDataExecCommand, "unrelated").await.unwrap();
            server_client = server_client.with_next_id(req.id);
            server_client.write_packet(PacketType::ServerDataAuthResponse, "").await.unwrap();
        });

        client.authenticate().await.unwrap();
        server.await.unwrap();
    }
}