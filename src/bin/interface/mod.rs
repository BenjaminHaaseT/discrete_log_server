use std::io::{Read, Write, BufRead, stdout};
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncRead, AsyncReadExt};
use termion::{raw::IntoRawMode, color, style, cursor, input, clear};

use discrete_log_server::{Response, BytesDeser, BytesSer, AsBytes};
use super::ClientError;

pub enum Interface {
    Init,
    Home,
}

impl Interface {
    pub fn new() -> Interface {
        Interface::Init
    }

    pub async fn receive_response<R: AsyncReadExt>(self, mut from_server: R) -> Result<Self, ClientError> {
        let mut stdout = stdout().into_raw_mode().expect("stdout unable to be converted into raw mode");
        match self {
            Interface::Init => {
                let response = Response::from_reader(&mut from_server)
                    .await
                    .map_err(|e| ClientError::Response(e))?;
                assert!(response.is_connection_ok());
                write!(
                    stdout, "{}{}{}{}{}",
                    cursor::Goto(1, 1), clear::CurrentLine, color::Fg(color::LightGreen),
                    "connection successful", color::Fg(color::Reset)
                )
                    .map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                todo!()
            }
            Interface::Home => {
                todo!()
            }
        }
    }
}