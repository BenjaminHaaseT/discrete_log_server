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
    Log,
    RSA,
    ReturnHome
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
                    "[q] - Quit ", "[:p:] - Check if p is prime ", "[l] - Solve discrete logarithm ", "[r] - Factor RSA public key "
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            Interface::Home => {
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
                    "[q] - Quit ", "[:p:] - Check if p is prime ", "[l] - Solve discrete logarithm ", "[r] - Factor RSA public key "
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            Interface::Prime => {
                // match on the response returned from the server
                match Response::from_reader(&mut from_server)
                    .await
                    .map_err(|e| ClientError::Response(e))?
                {
                    Response::Prime { p, prob } => {
                        write!(
                            stdout, "{}{}{}",
                            cursor::Goto(1, 5), color::Fg(color::Rgb(225, 247, 244)),
                            format!("{p} is prime with probability {prob:.10}, press any key to return to menu")
                        ).map_err(|e| ClientError::Write(e))?;
                        stdout.flush().map_err(|e| ClientError::Write(e))?;
                    }
                    Response::NotPrime { p} => {
                        write!(
                            stdout, "{}{}{}",
                            cursor::Goto(1, 5), color::Fg(color::Rgb(225, 247, 244)),
                            format!("{p} is not prime, pres any key to return to menu")
                        ).map_err(|e| ClientError::Write(e))?;
                        stdout.flush().map_err(|e| ClientError::Write(e))?;
                    }
                    _ => return Err(ClientError::IllegalResponse),
                }
                Ok(Interface::ReturnHome)
            }
            Interface::Log => {
                // clear the console for displaying the results of pollards method
                write!(
                    stdout, "{}{}{}",
                    cursor::Goto(1, 1), clear::All, color::Fg(color::Rgb(225, 247, 244))
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                // display table headings
                write!(
                    stdout, "{:<14}|{:^14}|{:^14}|{:^14}|{:^14}|{:^14}|{:^14}|\n",
                    "i", "x", "alpha", "beta", "y", "gamma", "delta"
                ).map_err(|e| ClientError::Write(e))?;
                write!(
                    stdout, "{}\n", "-".repeat(105)
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                // keep pulling responses from the server until they are finished
                loop {
                    match Response::from_reader(&mut from_server)
                        .await
                        .map_err(|e| ClientError::Response(e))?
                    {
                        Response::LogItem { item} => {
                            if item.xi != item.yi {
                                write!(
                                    stdout, "{:<14}|{:^14}|{:^14}|{:^14}|{:^14}|{:^14}|{:^14}|\n",
                                    item.i, item.xi, item.ai, item.bi, item.yi, item.gi, item.di
                                ).map_err(|e| ClientError::Write(e))?;
                                stdout.flush().map_err(|e| ClientError::Write(e))?;
                            } else {
                                write!(
                                    stdout, "{:<14}|{}{:^14}{}|{:^14}|{:^14}|{}{:^14}{}|{:^14}|{:^14}|\n",
                                    item.i, color::Fg(color::Rgb(31, 207, 31)), item.xi,
                                    color::Fg(color::Reset), item.ai, item.bi, color::Fg(color::Rgb(31, 207, 31)),
                                    item.yi,  color::Fg(color::Reset), item.gi, item.di
                                ).map_err(|e| ClientError::Write(e))?;
                                stdout.flush().map_err(|e| ClientError::Write(e))?;
                            }
                        }
                        Response::SuccessfulLog { log, g, h, p, ratio } => {
                            write!(
                                stdout, "{}{}{}\n{}\n",
                                style::Bold, "-".repeat(105), style::Reset,
                                format!("discrete log solved: {g}^{log} = {h} in the field F{p}, ratio of iterations to sqrt({p}) = {ratio:.10}")
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                stdout, "{}", "press any key to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        Response::UnsuccessfulLog { g, h, p} => {
                            write!(
                                stdout, "{}{}{}{}\n",
                                style::Bold, "-".repeat(105), style::Reset,
                                format!("discrete log unable to be solved for g: {g}, h: {h}, p: {p}")
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                stdout, "{}", "press any key to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        _ => return Err(ClientError::IllegalResponse),
                    }
                }
                Ok(Interface::ReturnHome)
            }
            Interface::RSA => {
                // clear the console for displaying the results of pollards method
                write!(
                    stdout, "{}{}{}",
                    cursor::Goto(1, 1), clear::All, color::Fg(color::Rgb(225, 247, 244))
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                // display table headings
                write!(
                    stdout, "{:<14}|{:^14}|{:^14}|{:^14}|\n",
                    "i", "x", "y", "g",
                ).map_err(|e| ClientError::Write(e))?;
                write!(
                    stdout, "{}\n", "-".repeat(60)
                ).map_err(|e| ClientError::Write(e))?;
                stdout.flush().map_err(|e| ClientError::Write(e))?;
                loop {
                    match Response::from_reader(&mut from_server)
                        .await
                        .map_err(|e| ClientError::Write(e))?
                    {
                        Response::RSAItem { item } => {
                            write!(
                                stdout, "{:<14}|{:^14}|{:^14}|{:^14}|\n",
                                item.i, item.xi, item.yi, item.g
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                        }
                        Response::SuccessfulRSA { p, q, ratio } => {
                            write!(
                                stdout, "{}{}{}\n{}\n",
                                style::Bold, "-".repeat(60), style::Reset,
                                format!("public key factored successfully: n = {} * {}, ratio of iterations to sqrt({}) {:.10}", p, q,  p * q, ratio)
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                stdout, "{}", "press any key to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        Response::UnsuccessfulRSA { n} => {
                            write!(
                                stdout, "{}{}{}\n{}\n",
                                style::Bold, "-".repeat(60), style::Reset,
                                format!("public key {n} not able to be factored")
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                stdout, "{}", "press any key to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;
                            stdout.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        _ => return Err(ClientError::IllegalResponse),
                    }
                }
                Ok(Interface::ReturnHome)
            }
            s => return Err(ClientError::InterfaceState(s)),
        }
    }

    pub async fn parse_request<W: AsyncWriteExt + Unpin, C: Read>(self, mut to_server: W, mut from_client: C) -> Result<Self, ClientError> {
        let mut stdout = stdout().into_raw_mode().expect("unable to convert terminal into raw mode");
        match self {
            Interface::Home => {
                let next_state = loop {
                    let mut buf = String::default();
                    let _ = from_client.read_to_string(&mut buf)
                        .map_err(|e| ClientError::Read(e))?;

                    match buf.to_lowercase().as_str() {
                        "q" => {
                            //TODO: log client exit here
                            break Interface::Quit;
                        }
                        p if !p.starts_with('-') && u64::from_str(p).is_ok() => {
                            let p = u64::from_str(p).expect("conversion to `u64` should not fail");
                            let frame = Frame::Prime { p };
                            to_server.write_all(frame.as_bytes().as_slice())
                                .await
                                .map_err(|e| ClientError::SendRequest(e))?;
                            break Interface::Prime;
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
                            break Interface::Log;
                        }
                        "r" => {
                            let modulus = utils::read_u64("modulus", &mut from_client, &mut stdout)?;
                            let exponent = utils::read_u64("exponent", &mut from_client, &mut stdout)?;

                            // create frame and send to server
                            let frame = Frame::RSA { n: modulus, e: exponent };
                            to_server.write_all(&frame.as_bytes())
                                .await
                                .map_err(|e| ClientError::SendRequest(e))?;
                            break Interface::RSA;
                        }
                        _ => utils::incorrect_input_prompt("please enter a valid option", &mut stdout)?,
                    }
                };
                Ok(next_state)
            }
            Interface::ReturnHome => {
                let mut buf = String::default();
                from_client.read_to_string(&mut buf)
                    .map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            s => Err(ClientError::InterfaceState(s))
        }
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
                Err(e) => incorrect_input_prompt("please enter a valid unsigned inter", out)?,
            }
        }
    }

    pub fn incorrect_input_prompt(prompt: &str, out: &mut RawTerminal<Stdout>) -> Result<(), ClientError> {
        write!(
            out, "{}{}{}{}{}",
            cursor::Goto(1, 4), color::Fg(color::Rgb(242, 217, 104)),
            clear::CurrentLine, prompt,
            color::Fg(color::Reset)
        ).map_err(|e| ClientError::Write(e))?;
        out.flush().map_err(|e| ClientError::Write(e))?;
        Ok(())
    }
}