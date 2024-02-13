use std::io::{Read, Write, BufRead, stdout};
use std::time::Duration;
use std::str::FromStr;
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncRead, AsyncReadExt};
use termion::{raw::IntoRawMode, color, style, cursor, input, clear};

use discrete_log_server::{Response, BytesDeser, BytesSer, AsBytes, Frame};
use super::ClientError;

pub enum Interface {
    Init,
    Home,
    Quit,
    Prime,
}

impl Interface {
    pub fn new() -> Interface {
        Interface::Init
    }

    pub async fn receive_response<R: AsyncReadExt + Unpin>(self, mut from_server: R) -> Result<Self, ClientError> {
        let mut stdout = stdout().into_raw_mode().expect("stdout unable to be converted into raw mode");
        match self {
            Interface::Init => {
                let response = Response::from_reader(&mut from_server)
                    .await
                    .map_err(|e| ClientError::Response(e))?;
                assert!(response.is_connection_ok());
                // Display home screen for client
                write!(
                    stdout,
                    "{}{}{}{}{:-^80}{}{}",
                    cursor::Goto(1, 1), cursor::Hide, clear::All, style::Bold, color::Fg(color::Rgb(92, 209, 193)), style::Reset,
                    "Pollards-Server\n"
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                // Display menu of options
                write!(
                    stdout, "{}{}{}\n{}\n{}\n{}\n",
                    cursor::Goto(1, 2), color::Fg(color::Rgb(225, 247, 244)),
                    "[q] - Quit", "[:p] - Check if p is prime", "[l] - Solve discrete logarithm", "[r] - Factor RSA public key"
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                // Display prompt to client
                write!(
                    stdout, "{}{}", cursor::Goto(1, 6), ">>> "
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            Interface::Home => {
                todo!()
            }
            _ => todo!(),
        }
    }

    pub async fn parse_request<W: AsyncWriteExt + Unpin, C: Read>(self, mut to_server: W, mut from_client: C) -> Result<Self, ClientError> {
        match self {
            Interface::Home => {
                let mut buf = String::default();
                let _ = from_client.read_to_string(&mut buf)
                    .map_err(|e| ClientError::Read(e))?;

                match buf.to_lowercase().as_str() {
                    "q" => {
                        //TODO: log client exit here
                        return Ok(Interface::Quit);
                    }
                    p if !p.starts_with('-') && u64::from_str(p).is_ok() => {
                        let p = u64::from_str(p).expect("conversion to `u64` should not fail");
                        let frame = Frame::Prime { p };
                        to_server.write_all(frame.as_bytes().as_slice())
                            .await
                            .map_err(|e| ClientError::SendRequest(e))?;
                        return Ok(Interface::Prime);
                    }
                    _ => todo!()
                }

            },
            _ => todo!()
        }

        todo!()
    }
}