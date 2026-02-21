use thiserror::Error;

#[derive(Debug, Error)]
pub enum RconError {
    #[error("io error: {0}")]
    Io(std::io::Error),

    #[error("utf8 error: {0}")]
    Utf8(std::string::FromUtf8Error),

    #[error("client error: {0}")]
    ClientError(String),

    #[error("operation timed out")]
    Timeout,

    #[error("authentication failed")]
    AuthFailed,

    #[error("did not conform to rcon protocol: {0}")]
    Protocol(String)
}


impl From<std::io::Error> for RconError {
    fn from(e: std::io::Error) -> Self { RconError::Io(e) }
}
impl From<std::string::FromUtf8Error> for RconError {
    fn from(e: std::string::FromUtf8Error) -> Self { RconError::Utf8(e) }
}
