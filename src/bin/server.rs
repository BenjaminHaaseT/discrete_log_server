//! The executable for running the server
use std::fmt::{Debug, Display};
use std::sync::{Arc};
use tokio::net::{ToSocketAddrs, TcpStream, TcpListener};
use tokio_stream::wrappers::TcpListenerStream;
use tokio::sync::{mpsc::{self, channel, Receiver, Sender}};
use tokio::task;
use tokio_util::sync::CancellationToken;
use tracing::{instrument, error, debug, info, warn};
use futures::{stream::{Stream, StreamExt}};
use uuid::Uuid;

use discrete_log_server::prelude::*;

/// The main accept loop for the server. Takes an address for the server will be bound to,
/// listens for incoming connections from clients and handles newly connected clients.
///
/// # Parameters
/// `server_addrs`, The address the server will be spawned to
///
/// # Returns
/// `Result<(), ServerError>`, `Ok(())` in the success case, otherwise `Err(ServerError)`.
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

/// The task that reads packets sent from the client.
///
/// Takes a socket and a sending half of a channel. Informs the broker of a new client connection and then begins
/// listening for incoming packets sent by the client.
///
/// # Parameters
/// `socket`, The socket that the client will send packets over
/// `broker_send`, The sending half of the channel to send parsed events to
///
/// # Returns
/// `Result<(), ServerError>`, `Ok(())` in the success case otherwise `Err(ServerError)`.
#[instrument(ret, err, skip(broker_send), fields(peer_addr = ?socket.peer_addr()))]
async fn client_read_task(socket: TcpStream, broker_send: Sender<Event>) -> Result<(), ServerError> {
    // Split the socket into reader and writer
    let (mut client_reader, client_writer) = socket.into_split();
    // unique id for the client
    let peer_id = Uuid::new_v4();
    // Cancellation token for graceful shutdown
    let token = CancellationToken::new();

    // Create new client event to inform broker
    let event = Event::NewClient {
        peer_id,
        socket: client_writer,
        token
    };

    // Send the event to the broker
    broker_send.send(event)
        .await
        .map_err(|_e| ServerError::ChannelSend(format!("Client {} unable to send event to broker", peer_id)))?;

    // loop {
    //     let frame = match
    // }

    todo!()
}

#[derive(Debug)]
pub enum ServerError<> {
    Connection(std::io::Error),
    ChannelSend(String)
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            _ => write!(f, "")
        }
    }
}

impl<T> std::error::Error for ServerError {}

fn main() {}


