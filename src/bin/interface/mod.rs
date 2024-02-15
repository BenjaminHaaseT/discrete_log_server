use std::io::{Read, Write, BufRead, stdout, Stdin, Stdout};
use std::time::Duration;
use std::str::FromStr;
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncRead, AsyncReadExt};
pub use termion::{raw::{IntoRawMode, RawTerminal}, color, style, cursor, input, clear};

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
                    stdout, "{}{}{}{}{}{}\n",
                    cursor::Goto(1, 5), color::Fg(color::Rgb(225, 247, 244)),
                    "[q] - Quit", "[:p] - Check if p is prime", "[l] - Solve discrete logarithm", "[r] - Factor RSA public key"
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                // // Display prompt to client
                // write!(
                //     stdout, "{}{}", cursor::Goto(1, 6), ">>> "
                // ).map_err(|e| ClientError::Write(e))?;
                // stdout.flush().map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            Interface::Home => {
                todo!()
            }
            _ => todo!(),
        }
    }

    pub async fn parse_request<W: AsyncWriteExt + Unpin, C: Read>(self, mut to_server: W, mut from_client: C) -> Result<Self, ClientError> {
        let mut stdout = stdout().into_raw_mode().expect("unable to convert terminal into raw mode");
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
                    "l" => {
                        let base = utils::read_u64("base", &mut from_client, &mut stdout)?;
                        let val = utils::read_u64("value", &mut from_client, &mut stdout)?;
                        let prime = utils::read_u64("prime", &mut from_client, &mut stdout)?;

                        // create frame and send to server
                        let frame = Frame::Log { g: base, h: val, p: prime };
                        to_server.write_all(&frame.as_bytes())
                            .await
                            .map_err(|e| ClientError::SendRequest(e))?;
                    }
                    _ => todo!()
                }

            },
            _ => todo!()
        }

        todo!()
    }
}


mod utils {
    use super::*;
    pub fn read_u64<'a, C: Read>(label: &'a str, from_client: &mut C, out: &mut RawTerminal<Stdout>) -> Result<u64, ClientError> {
        loop {
            write!(
                out, "{}{}{}",
                cursor::Goto(1, 5), clear::CurrentLine, format!("enter {}: ", label),
            ).map_err(|e| ClientError::Write(e))?;
            out.flush().map_err(|e| ClientError::Write(e))?;

            let mut buf = String::default();
            from_client.read_to_string(&mut buf)
                .map_err(|e| ClientError::Read(e))?;

            match u64::from_str(buf.trim_end_matches('\n')) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    write!(
                        out, "{}{}{}{}{}",
                        cursor::Goto(1, 4), color::Fg(color::Rgb(242, 217, 104)),
                        clear::CurrentLine, "please enter a valid unsigned integer",
                        color::Fg(color::Reset)
                    ).map_err(|e| ClientError::Write(e))?;
                    out.flush().map_err(|e| ClientError::Write(e))?;
                }
            }
        }
    }
}