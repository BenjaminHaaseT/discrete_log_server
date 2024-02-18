use std::io::{self, stdin, Read};
use std::net::SocketAddr;
use tokio::net::{TcpStream};
use tokio::io as tokio_io;
use tokio_io::{AsyncReadExt, AsyncWriteExt};
use crate::interface::Interface;

mod interface;

pub enum ClientError {
    Response(io::Error),
    Write(io::Error),
    Read(io::Error),
    SendRequest(tokio_io::Error),
    IllegalResponse,
    InterfaceState(Interface),
    Connection(io::Error),
}

struct Client;

impl Client {
    async fn connect(addr: SocketAddr) -> Result<(), ClientError> {
        // create interface
        let mut interface = Interface::new();

        // handle to standard input
        let mut stdin = stdin();

        // connect to server
        let server_socket = TcpStream::connect(addr)
            .await
            .map_err(|e| ClientError::Connection(e))?;
        let (mut from_server, mut to_server) = server_socket.into_split();

        // main loop for the ui
        loop {
            interface = interface.receive_response(&mut from_server).await?;
            interface = match interface.parse_request(&mut to_server, &mut stdin).await {
                Ok(Interface::Quit) => {
                    // TODO: log exiting application
                    break;
                }
                Ok(i) => i,
                Err(e) => return Err(e),
            };
        }

        Ok(())
    }
}
fn main() {
    todo!()
}