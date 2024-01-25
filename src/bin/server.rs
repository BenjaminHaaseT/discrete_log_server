//! The executable for running the server
use std::fmt::{Debug, Display};
use std::sync::{Arc};
use tokio::net::{ToSocketAddrs, TcpStream, TcpListener};
use tokio_stream::wrappers::TcpListenerStream;
use tokio::sync::{mpsc::{self, channel, Receiver, Sender}};
use tokio::task::{self, JoinError, JoinHandle};
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{instrument, error, debug, info, warn};
use futures::{stream::{Stream, StreamExt}};
use tokio::net::tcp::OwnedWriteHalf;
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
    let mut broker_handle = task::spawn(main_broker(broker_recv));
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

    // TODO: graceful shutdown routine
    broker_handle
        .await
        .map_err(|e| ServerError::Task(e))??;
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
    let shutdown_token = token.child_token();
    let _token = token.drop_guard();

    // Create new client event to inform broker a new client has connected
    let event = Event::NewClient {
        peer_id,
        socket: client_writer,
        token: shutdown_token,
    };

    // Send the event to the broker
    broker_send.send(event)
        .await
        .map_err(|_e| ServerError::ChannelSend(format!("Client {} unable to send event to broker", peer_id)))?;

    loop {
        let frame = Frame::from_reader(&mut client_reader)
            .await
            .map_err(|e| ServerError::Read(e))?;

        // Match on frame
        let event = match frame {
            Frame::Log { g, h, p } => Event::Log { peer_id, g, h, p },
            Frame::RSA { n, e} => Event::RSA { peer_id, n, e },
            Frame::Quit => {
                // The client is quitting the application, so break
                broker_send.send(Event::Quit { peer_id })
                    .await
                    .map_err(|_e| ServerError::ChannelSend("Client {} unable to send event to main broker".to_string()))?;
                info!(peer_id = ?peer_id, "Client {} read task is exiting loop", peer_id);
                break;
            },
            f => {
                // Create error for logging, this case should not happen
                let error = ServerError::IllegalFrame(peer_id, f);
                error!(error = ?error, "Illegal frame received from client {}", peer_id);
                return Err(error);
            }
        };

        // Send the event to the broker
        broker_send.send(event)
            .await
            .map_err(|_e| ServerError::ChannelSend("Client {} unable to send event to main broker".to_string()))?;
    }

    // _token will be dropped after task finishes, sending a shutdown signal to the write task
    Ok(())
}

/// The task that will write responses back to the client.
///
/// Takes a write half of socket, a receiving half of a channel for receiving responses from the broker and a token
/// for listening to shutdown signals sent from the associated writer task. This function will listen fo incoming
/// responses from the broker and write them back to the client's socket.
///
/// # Parameters
/// `peer_id`, The `Uuid` of the client
/// `client_writer`, The write half of the client's socket
/// `broker_recv`, The receiving half of the channel connecting this task with the main broker
/// `token`, The `CancellationToken` that informs this task to shutdown
///
/// # Returns
/// `Result<(), ServerError>`, In the success case a `Ok(())` will be returned, otherwise `Err(ServerError)`.
// #[instrument(ret, err, skip(client_writer, broker_recv, token))]
// async fn client_write_task(peer_id: Uuid, client_writer: OwnedWriteHalf, broker_recv: Receiver<Response>, token: CancellationToken) -> Result<(), ServerError> {
//     todo!()
// }

async fn main_broker(events: Receiver<Event>) -> Result<(), ServerError> {
    todo!()
}

#[derive(Debug)]
pub enum ServerError<> {
    Connection(std::io::Error),
    ChannelSend(String),
    IllegalFrame(Uuid, Frame),
    Read(std::io::Error),
    Task(JoinError),
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


