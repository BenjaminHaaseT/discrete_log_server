use std::io::{self, stdin, Read};
use std::fmt;
use std::net::SocketAddr;
use tokio::net::{TcpStream};
// use tokio::task;
use tokio::runtime;
use tokio::io as tokio_io;
use tokio_io::{AsyncReadExt, AsyncWriteExt};
use tracing::instrument;
use crate::interface::Interface;

mod interface;

#[derive(Debug)]
pub enum ClientError {
    Response(io::Error),
    Write(io::Error),
    Read(io::Error),
    SendRequest(tokio_io::Error),
    IllegalResponse,
    InterfaceState(Interface),
    Connection(io::Error),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Response(e) => write!(f, "{e}"),
            ClientError::Write(e) => write!(f, "{e}"),
            ClientError::Read(e) => write!(f, "{e}"),
            ClientError::SendRequest(e) => write!(f, "{e}"),
            ClientError::IllegalResponse => write!(f, "illegal response received from server"),
            ClientError::InterfaceState(i) => write!(f, "interface entered illegal state: {i:?}"),
            ClientError::Connection(e) => write!(f, "{e}"),
        }
    }
}

/// A struct for connecting to the server
struct Client;

impl Client {

    /// Connects to the server at the address given by `addr`.
    #[instrument(ret, err)]
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
    let addr = ([127, 0, 0, 1], 8080).into();
    let mut rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("unable to build runtime");
    if let Err(e) = rt.block_on(Client::connect(addr)) {
        eprintln!("{e}");
    }
}