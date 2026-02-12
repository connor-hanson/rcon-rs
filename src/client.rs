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
        let buf = build_packet(id, packet_type.clone(), body);
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
    /// We will send an empty command with a unique ID as a "terminator".
    /// We then read until we see this ID. 
    pub async fn exec(&mut self, command: &str) -> Result<String, RconError> {
        log::debug!("Received command: {}", command);

        let cmd_id = self.alloc_id();
        self.write_packet(cmd_id, PacketType::ServerDataExecCommand, command).await?;


        let terminator = self.alloc_id();
        self.write_packet(terminator, PacketType::ServerDataExecCommand, "_").await?;

        let mut out = String::new();

        loop {
            let packet = self.read_packet_timed().await?;
            if packet.id == terminator {
                break;
            }

            if packet.id != cmd_id {
                log::debug!("ignoring unrelated packet: {:?}", packet);
                continue;
            }

            match packet.packet_type {
                PacketType::ServerDataExecCommand => out.push_str(&packet.body),
                PacketType::ServerDataResponseValue => out.push_str(&packet.body),
                _ => ()
            }
        }

        Ok(out)
    }
}