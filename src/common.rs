#[derive(Debug, PartialEq, Clone)]
pub enum PacketType {
    ServerDataResponseValue,
    ServerDataAuthResponse,
    ServerDataExecCommand,
    ServerDataAuth,
}

impl Into<i32> for PacketType {
    fn into(self) -> i32 {
        match self {
            PacketType::ServerDataAuth => 3,
            PacketType::ServerDataAuthResponse => 2,
            PacketType::ServerDataExecCommand => 2,
            PacketType::ServerDataResponseValue => 0,
        }
    }
}

impl PacketType {
    pub fn from_i32(value: i32, is_auth: bool) -> PacketType {
        match value {
            3 => PacketType::ServerDataAuth,
            2 if is_auth => PacketType::ServerDataAuthResponse,
            2 => PacketType::ServerDataExecCommand,
            0 => PacketType::ServerDataResponseValue,
            _ => panic!("Unexpected value to parse")
        }
    }
}