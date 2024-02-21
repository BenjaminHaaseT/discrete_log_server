use std::io::{Read, Write, BufRead, stdout, Stdin, Stdout};
use std::time::Duration;
use std::str::FromStr;
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncRead, AsyncReadExt};
use tracing::{error, info, debug, instrument};
pub use termion::{raw::{IntoRawMode, RawTerminal}, color, screen::{AlternateScreen, IntoAlternateScreen}, style, cursor, input::TermRead, event::Key, clear};

use discrete_log_server::{Response, BytesDeser, BytesSer, AsBytes, Frame};
use super::ClientError;

/// The interface for client interactions with the server
///
/// This struct will manage the parsing of requests from client input, sending requests to the server,
/// and receiving responses from the server as well. The `Interface` type is a state machine, that will
/// change state based on input received from the client as well as responses received from the server.
pub enum Interface {
    Init,
    Home,
    Quit,
    Prime,
    Log,
    RSA,
    ReturnHome { row: u16, alt_screen: Option<AlternateScreen<Stdout>> }
}

impl Interface {
    pub fn new() -> Interface {
        Interface::Init
    }

    /// Transitions the state of the Interface based on the response received from the server.
    pub async fn receive_response<R: AsyncReadExt + Unpin>(self, mut from_server: R) -> Result<Self, ClientError> {
        let mut out = stdout().into_raw_mode().expect("stdout unable to be converted into raw mode");
        match self {
            Interface::Init => {
                debug!("interface is in `Init` state");
                let response = Response::from_reader(&mut from_server)
                    .await
                    .map_err(|e| ClientError::Response(e))?;
                assert!(response.is_connection_ok());
                info!("successfully connected to server");
                // Display home screen for client
                write!(
                    out,
                    "{}{}{}{}{}{}{:-^80}{}",
                    cursor::Goto(1, 1), cursor::Hide, clear::BeforeCursor, clear::AfterCursor, style::Bold, color::Fg(color::Rgb(92, 209, 193)), "Pollards-Server", style::Reset,
                ).map_err(|e| ClientError::Write(e))?;
                out.flush().map_err(|e| ClientError::Write(e))?;

                // Display menu of options
                write!(
                    out, "{}{}{}{}{}{}",
                    cursor::Goto(1, 5), color::Fg(color::Rgb(225, 247, 244)),
                    "[q] - Quit ", "[:p:] - Check if p is prime ", "[l] - Solve discrete logarithm ", "[r] - Factor RSA public key "
                ).map_err(|e| ClientError::Write(e))?;
                out.flush().map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            Interface::Home => {
                debug!("interface is in `Home` state");
                // Display home screen for client
                write!(
                    out,
                    "{}{}{}{}{}{}{:-^80}{}{}",
                    cursor::Goto(1, 1), cursor::Hide, clear::BeforeCursor, clear::AfterCursor, style::Bold, color::Fg(color::Rgb(92, 209, 193)),
                    "Pollards-Server", style::Reset, color::Fg(color::Reset)
                ).map_err(|e| ClientError::Write(e))?;
                out.flush().map_err(|e| ClientError::Write(e))?;
                // Display menu of options
                write!(
                    out, "{}{}{}{}{}{}",
                    cursor::Goto(1, 5), color::Fg(color::Rgb(225, 247, 244)),
                    "[q] - Quit ", "[:p:] - Check if p is prime ", "[l] - Solve discrete logarithm ", "[r] - Factor RSA public key "
                ).map_err(|e| ClientError::Write(e))?;
                out.flush().map_err(|e| ClientError::Write(e))?;
                Ok(Interface::Home)
            }
            Interface::Prime => {
                debug!("interface is in `Prime` state");
                // match on the response returned from the server
                match Response::from_reader(&mut from_server)
                    .await
                    .map_err(|e| ClientError::Response(e))?
                {
                    Response::Prime { p, prob } => {
                        write!(
                            out, "{}{}{}{}",
                            cursor::Goto(1, 5), clear::CurrentLine, color::Fg(color::Rgb(225, 247, 244)),
                            format!("{p} is prime with probability {prob:.10}, press enter to return to menu")
                        ).map_err(|e| ClientError::Write(e))?;
                        out.flush().map_err(|e| ClientError::Write(e))?;
                    }
                    Response::NotPrime { p} => {
                        write!(
                            out, "{}{}{}{}",
                            cursor::Goto(1, 5), clear::CurrentLine, color::Fg(color::Rgb(225, 247, 244)),
                            format!("{p} is not prime, press enter to return to menu")
                        ).map_err(|e| ClientError::Write(e))?;
                        out.flush().map_err(|e| ClientError::Write(e))?;
                    }
                    _ => return Err(ClientError::IllegalResponse),
                }
                Ok(Interface::ReturnHome { row: 6, alt_screen: None })
            }
            Interface::Log => {
                // For writing to a new screen, that way we don't pollute the main screen when output
                // becomes long
                let mut alt_out = stdout()
                    .into_alternate_screen()
                    .map_err(|e| ClientError::Write(e))?;
                debug!("interface is in `Log` state");

                // clear the console for displaying the results of pollards method
                write!(
                    alt_out, "{}{}{}{}",
                    cursor::Goto(1, 1), clear::BeforeCursor, clear::AfterCursor, color::Fg(color::Rgb(225, 247, 244))
                ).map_err(|e| ClientError::Write(e))?;
                alt_out.flush().map_err(|e| ClientError::Write(e))?;

                // display table headings
                write!(
                    alt_out, "{:<11}|{:^11}|{:^11}|{:^11}|{:^11}|{:^11}|{:^11}|\n",
                    "i", "x", "alpha", "beta", "y", "gamma", "delta"
                ).map_err(|e| ClientError::Write(e))?;
                alt_out.flush().map_err(|e| ClientError::Write(e))?;

                write!(
                    alt_out, "{}{}\n", cursor::Goto(1, 2), "-".repeat(84)
                ).map_err(|e| ClientError::Write(e))?;
                alt_out.flush().map_err(|e| ClientError::Write(e))?;

                // Keep track of what row we are on
                let mut row = 3;

                // keep pulling responses from the server until they are finished
                loop {
                    match Response::from_reader(&mut from_server)
                        .await
                        .map_err(|e| ClientError::Response(e))?
                    {
                        Response::LogItem { item} => {
                            if item.xi != item.yi {
                                write!(
                                    alt_out, "{}{:<11}|{:^11}|{:^11}|{:^11}|{:^11}|{:^11}|{:^11}|\n",
                                    cursor::Goto(1, row), item.i, item.xi, item.ai, item.bi, item.yi, item.gi, item.di
                                ).map_err(|e| ClientError::Write(e))?;
                                alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            } else {
                                write!(
                                    alt_out, "{}{:<11}|{}{:^11}{}|{:^11}|{:^11}|{}{:^11}{}|{:^11}|{:^11}|\n",
                                    cursor::Goto(1, row), item.i, color::Fg(color::Rgb(31, 207, 31)), item.xi,
                                    color::Fg(color::Rgb(225, 247, 244)), item.ai, item.bi, color::Fg(color::Rgb(31, 207, 31)),
                                    item.yi,  color::Fg(color::Rgb(225, 247, 244)), item.gi, item.di
                                ).map_err(|e| ClientError::Write(e))?;
                                alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            }
                            row += 1;
                        }
                        Response::SuccessfulLog { log, g, h, p, ratio } => {
                            write!(
                                alt_out, "{}{}{}{}\n",
                                cursor::Goto(1, row), style::Bold, "-".repeat(84), style::Reset,
                                // cursor::Goto(1, row + 1),
                                // format!("discrete log solved: {g}^{log} = {h} in the field F{p}, ratio of iterations to sqrt({p}) = {ratio:.10}")
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                alt_out, "{}{}{}\n",
                                cursor::Goto(1, row + 1), color::Fg(color::Rgb(225, 247, 244)),
                                format!("discrete log solved: {g}^{log} = {h} in the field F{p}, ratio of iterations to sqrt({p}) = {ratio:.10}")
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                alt_out, "{}{}", cursor::Goto(1, row + 2), "press enter to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        Response::UnsuccessfulLog { g, h, p} => {
                            write!(
                                alt_out, "{}{}{}{}\n{}{}\n",
                                cursor::Goto(1, row), style::Bold, "-".repeat(84), style::NoBold,
                                cursor::Goto(1, row + 1),
                                format!("discrete log unable to be solved for g: {g}, h: {h}, p: {p}")
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            write!(
                                alt_out, "{}", "press enter to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        _ => return Err(ClientError::IllegalResponse),
                    }
                }
                Ok(Interface::ReturnHome { row: row + 3, alt_screen: Some(alt_out) })
            }
            Interface::RSA => {
                let mut alt_out = stdout().into_alternate_screen()
                    .map_err(|e| ClientError::Write(e))?;

                debug!("interface is in `RSA` state");

                // clear the console for displaying the results of pollards method
                write!(
                    alt_out, "{}{}{}",
                    cursor::Goto(1, 1), clear::All, color::Fg(color::Rgb(225, 247, 244))
                ).map_err(|e| ClientError::Write(e))?;
                alt_out.flush().map_err(|e| ClientError::Write(e))?;

                // display table headings
                write!(
                    alt_out, "{:<14}|{:^14}|{:^14}|{:^14}|\n",
                    "i", "x", "y", "g",
                ).map_err(|e| ClientError::Write(e))?;
                write!(
                    alt_out, "{}\n", "-".repeat(60)
                ).map_err(|e| ClientError::Write(e))?;
                alt_out.flush().map_err(|e| ClientError::Write(e))?;

                loop {
                    match Response::from_reader(&mut from_server)
                        .await
                        .map_err(|e| ClientError::Write(e))?
                    {
                        Response::RSAItem { item } => {
                            write!(
                                alt_out, "{:<14}|{:^14}|{:^14}|{:^14}|\n",
                                item.i, item.xi, item.yi, item.g
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                        }
                        Response::SuccessfulRSA { p, q, ratio } => {
                            write!(
                                alt_out, "{}{}{}\n{}\n",
                                style::Bold, "-".repeat(60), style::Reset,
                                format!("public key factored successfully: n = {} * {}, ratio of iterations to sqrt({}) {:.10}", p, q,  p * q, ratio)
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;

                            write!(
                                alt_out, "{}", "press any key to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;

                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        Response::UnsuccessfulRSA { n} => {
                            write!(
                                alt_out, "{}{}{}\n{}\n",
                                style::Bold, "-".repeat(60), style::Reset,
                                format!("public key {n} not able to be factored")
                            ).map_err(|e| ClientError::Write(e))?;
                            alt_out.flush().map_err(|e| ClientError::Write(e))?;

                            write!(
                                alt_out, "{}", "press any key to return to menu "
                            ).map_err(|e| ClientError::Write(e))?;

                            alt_out.flush().map_err(|e| ClientError::Write(e))?;
                            break;
                        }
                        _ => return Err(ClientError::IllegalResponse),
                    }
                }
                Ok(Interface::ReturnHome { row: 6, alt_screen: Some(alt_out) })
            }
            s => return Err(ClientError::InterfaceState),
        }
    }

    /// Transitions the state of the interface based on the input of the client
    pub async fn parse_request<W: AsyncWriteExt + Unpin, C: Read>(self, mut to_server: W, mut from_client: C) -> Result<Self, ClientError> {
        let mut stdout = stdout().into_raw_mode().expect("unable to convert terminal into raw mode");
        match self {
            Interface::Home => {
                debug!("interface is in `Home` state");
                let next_state = loop {
                    // let mut buf = String::default();
                    // let _ = from_client.read_to_string(&mut buf)
                    //     .map_err(|e| ClientError::Read(e))?;
                    let buf = utils::read_client_input(&mut stdout, 6, 1)?;

                    match buf.to_lowercase().as_str() {
                        "q" => {
                            info!("client exiting");
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
            Interface::ReturnHome { row, alt_screen } => {
                debug!("interface is in `ReturnHome` state");
                let _ = if let Some(mut alt_out) = alt_screen {
                    utils::read_client_input(&mut alt_out, row, 1)
                } else {
                    utils::read_client_input(&mut stdout, row, 1)
                };
                Ok(Interface::Home)
            }
            s => Err(ClientError::InterfaceState)
        }
    }
}

mod utils {
    use super::*;
    use std::io::{stdin, Read};
    pub fn read_u64<'a, C: Read>(label: &'a str, from_client: &mut C, out: &mut RawTerminal<Stdout>) -> Result<u64, ClientError> {
        let prompt = format!("enter {}: ", label);
        loop {
            write!(
                out, "{}{}{}",
                cursor::Goto(1, 5), clear::CurrentLine, prompt,
            ).map_err(|e| ClientError::Write(e))?;
            out.flush().map_err(|e| ClientError::Write(e))?;

            // let mut buf = String::default();
            // from_client.read_to_string(&mut buf)
            //     .map_err(|e| ClientError::Read(e))?;
            let buf = read_client_input(out, 5, prompt.len() as u16)?;

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

    pub fn read_client_input<W: Write>(out: &mut W, row: u16, col: u16) -> Result<String, ClientError> {
        let mut keys = stdin().keys();
        let mut buf = String::default();

        loop {
            match keys.next() {
                Some(Ok(Key::Char('\n'))) => {
                    write!(
                        out, "{}{}", cursor::Goto(1, row), clear::CurrentLine
                    ).map_err(|e| ClientError::Write(e))?;
                    out.flush().map_err(|e| ClientError::Write(e))?;
                    break;
                },
                Some(Ok(Key::Backspace)) => {
                    if let Some(_) = buf.pop() {
                        write!(
                            out, "{}{}", cursor::Left(1), clear::AfterCursor
                        ).map_err(|e| ClientError::Write(e))?;
                        out.flush().map_err(|e| ClientError::Write(e))?;
                    }
                }
                Some(Ok(Key::Char(c))) => {
                    write!(
                        out, "{}{}", cursor::Goto(col + buf.len() as u16, row), c
                    ).map_err(|e| ClientError::Write(e))?;
                    out.flush().map_err(|e| ClientError::Write(e))?;
                    buf.push(c);
                }
                Some(Err(e)) => return Err(ClientError::Write(e)),
                _ => {}
            }
        }


        Ok(buf)
    }
}