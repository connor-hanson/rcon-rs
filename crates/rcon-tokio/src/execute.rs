use tokio::{io::{AsyncRead, AsyncWrite}, net::TcpStream, time::timeout};

use crate::{client::RconClient, common::PacketType, errors::RconError};


impl RconClient<TcpStream> {
    pub async fn execute(&mut self, command: &str) -> Result<String, RconError> {
        const MAX_BODY_SIZE: usize = 511;
        
        if command.len() <= MAX_BODY_SIZE {
            return self.execute_with_retry(command).await;
        }

        log::warn!("Command exceeds {} bytes ({}), splitting into {} chunks", 
            MAX_BODY_SIZE, command.len(), (command.len() + MAX_BODY_SIZE - 1) / MAX_BODY_SIZE);

        let chunks: Vec<&str> = command
            .as_bytes()
            .chunks(MAX_BODY_SIZE)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
            .collect();

        let mut results = Vec::new();
        for chunk in chunks {
            results.push(self.execute_with_retry(chunk).await?);
        }
        Ok(results.join(""))
    }

    async fn execute_with_retry(&mut self, command: &str) -> Result<String, RconError> {
        for attempt in 0..self.client_config.max_reconnect_attempts {
            log::debug!("Executing command with attempt {}/{}", attempt + 1, self.client_config.max_reconnect_attempts);
            match self._execute(command).await {
                Ok(result) => return Ok(result),
                Err(e) => log::warn!("Failed to execute command on attempt {}/{}. Error: {:?}", attempt + 1, self.client_config.max_reconnect_attempts, e),
            }

            if self.client_config.auto_reconnect && attempt < self.client_config.max_reconnect_attempts {
                log::warn!("Attempting to reconnect client and retry command execution");
                *self = RconClient::connect(self.client_config.clone()).await?;
            } else {
                break;
            }
        }

        Err(RconError::ClientError(format!("Failed to execute command after {} attempts", self.client_config.max_reconnect_attempts)))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> RconClient<S> {
    async fn _execute(&mut self, command: &str) -> Result<String, RconError> {
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
                io_timeout: Duration::from_secs(1),
                ..Default::default()
            });

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream)
                .with_client_config(RconClientConfig {
                    io_timeout: Duration::from_secs(1),
                    ..Default::default()
                });
            
            let cmd = server_client.read_packet().await.unwrap();
            assert_eq!(cmd.packet_type, PacketType::ServerDataExecCommand);
            assert_eq!(cmd.body, "cmd");

            server_client = server_client.with_next_id(EXPECTED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "hello ").await.unwrap();
            server_client = server_client.with_next_id(EXPECTED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "world").await.unwrap();
            tokio::time::sleep(TIMEOUT * 2).await;
        });

        let out = client._execute("cmd").await.unwrap();
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
                io_timeout: Duration::from_secs(1),
                ..Default::default()
            });

        let server = tokio::spawn(async move {
            let mut server_client = RconClient::new(server_stream)
                .with_client_config(RconClientConfig {
                    io_timeout: Duration::from_secs(1),
                    ..Default::default()
                });
            
            let cmd = server_client.read_packet().await.unwrap();
            assert_eq!(cmd.packet_type, PacketType::ServerDataExecCommand);
            assert_eq!(cmd.body, "cmd");

            server_client = server_client.with_next_id(UNRELATED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "unrelated").await.unwrap();
            server_client = server_client.with_next_id(EXPECTED_ID);
            server_client.write_packet(PacketType::ServerDataExecCommand, "hello world").await.unwrap();
            tokio::time::sleep(TIMEOUT * 2).await;
        });

        let out = client._execute("cmd").await.unwrap();
        assert_eq!(out, "hello world");
        server.await.unwrap();
    }
}