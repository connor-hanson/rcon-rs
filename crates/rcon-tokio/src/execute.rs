use tokio::{io::{AsyncRead, AsyncWrite}, time::timeout};

use crate::{client::RconClient, common::PacketType, errors::RconError};

impl<S: AsyncRead + AsyncWrite + Unpin> RconClient<S> {
    pub async fn execute(&mut self, command: &str) -> Result<String, RconError> {
        log::debug!("Executing command: {:?}", command);
        let cmd_id = self.write_packet(PacketType::ServerDataExecCommand, command).await?;

        let mut out = String::new();
        let mut data_seen = false;

        loop {
            match timeout(self.client_config.idle_timeout, self.read_packet()).await {
                Ok(Ok(pkt)) => {
                    if pkt.id != cmd_id {
                        log::debug!("Received packet with id {:?} while waiting for response to command with id {:?}, ignoring", pkt.id, cmd_id);
                        continue;
                    }

                    data_seen = true;
                    let ptype: i32 = pkt.packet_type.into();
                    match ptype {
                        0 => out.push_str(&pkt.body),
                        2 => out.push_str(&pkt.body),
                        _ => log::debug!(
                            "Received packet with unexpected type {:?} while waiting for command response, ignoring", 
                            pkt.packet_type
                        )
                    }
                },
                Ok(Err(e)) => {
                    return Err(e)
                },
                Err(_) => {
                    if data_seen {
                        log::debug!("Idle timeout reached while waiting for more data, returning response");
                        break;
                    }

                    log::debug!("Idle timeout reached without receiving any data, returning empty response");
                    break;
                }
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::RconClientConfig;

    use super::*;
    use tokio::io::duplex;

    const MAX_BUFFER_SIZE: usize = 4096;

    #[tokio::test]
    async fn exec_aggregates_multiple_packets_then_stops_on_idle() {
        const TIMEOUT: Duration = Duration::from_millis(100);
        const EXPECTED_ID: i32 = 1;

        let (client_stream, server_stream) = duplex(MAX_BUFFER_SIZE);
        let mut client = RconClient::new(client_stream)
            .with_client_config(RconClientConfig {
                idle_timeout: TIMEOUT,
                ..Default::default()
            });

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream);
            
            let cmd = server_client.read_packet().await.unwrap();
            assert_eq!(cmd.packet_type, PacketType::ServerDataExecCommand);
            assert_eq!(cmd.body, "cmd");

            server_client = server_client.with_next_id(EXPECTED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "hello ").await.unwrap();
            server_client = server_client.with_next_id(EXPECTED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "world").await.unwrap();
            tokio::time::sleep(TIMEOUT * 2).await;
        });

        let out = client.execute("cmd").await.unwrap();
        assert_eq!(out, "hello world");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn exec_ignores_unrelated_packet_ids() {
        const TIMEOUT: Duration = Duration::from_millis(100);
        const EXPECTED_ID: i32 = 1;
        const UNRELATED_ID: i32 = 2;

        let (client_stream, server_stream) = duplex(MAX_BUFFER_SIZE);
        let mut client = RconClient::new(client_stream)
            .with_client_config(RconClientConfig {
                idle_timeout: TIMEOUT,
                ..Default::default()
            });

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream);
            
            let cmd = server_client.read_packet().await.unwrap();
            assert_eq!(cmd.packet_type, PacketType::ServerDataExecCommand);
            assert_eq!(cmd.body, "cmd");

            server_client = server_client.with_next_id(UNRELATED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "unrelated").await.unwrap();
            server_client = server_client.with_next_id(EXPECTED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "hello world").await.unwrap();
            tokio::time::sleep(TIMEOUT * 2).await;
        });

        let out = client.execute("cmd").await.unwrap();
        assert_eq!(out, "hello world");
        server.await.unwrap();
    }
}