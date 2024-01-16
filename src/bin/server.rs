//! The executable for running the server
use std::fmt::{Debug, Display};
use std::sync::{Arc};
use tokio::net::{ToSocketAddrs, TcpStream, TcpListener};
use tokio_stream::wrappers::TcpListenerStream;
use tokio::sync::{mpsc::{channel, Receiver, Sender}};
use tokio::task;
use tracing::{instrument, error, debug, info, warn};
use futures::{stream::{Stream, StreamExt}};


/// The main accept loop for the server. Takes an address for the server will be bound to,
/// listens for incoming connections from clients and handles newly connected clients.
///
/// # Parameters
/// `server_addrs`: The address the server will be spawned to
///
/// # Returns
/// `Result<(), ServerError>`: `Ok(())` in the success case, otherwise `Err(ServerError)`.
#[instrument(ret, err)]
async fn accept_loop(server_addrs: impl ToSocketAddrs + Debug + Clone, buf_size: usize) -> Result<(), ServerError> {
    // Bind to the given server address
    let mut listener = TcpListenerStream::new(TcpListener::bind(server_addrs)
        .await
        .map_err(|e| ServerError::Connection(e))?);
    debug!("bound to address successfully");

    // Channel for connecting to main broker task
    let (broker_send, broker_recv) = channel::<Event>(buf_size);

    // Spawn broker task
    let _broker_handle = task::spawn(main_broker(broker_recv));
    debug!("broker task spawned");

    // Accept loop
    while let Some(socket_res) = listener.next().await {
        // Parse the result
        match socket_res {
            Ok(socket) => {
                info!(peer_addr = ?socket.peer_addr(), "Accepting {:?}", socket.peer_addr());
                task::spawn(client_read_task(socket, broker_send.clone()));
            }
            Err(e) => error!(error = ?e, "Unable to accept client"),
        }
    }

    //TODO: graceful shutdown routine

    Ok(())
}

#[derive(Debug)]
pub enum ServerError {
    Connection(std::io::Error),
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            _ => write!(f, "")
        }
    }
}

impl std::error::Error for ServerError {}

fn main() {}


