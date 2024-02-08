//! The executable for running the server
use std::fmt::{Debug, Display};
use std::collections::HashMap;
use std::sync::{Arc};
use rand;
use rand::Rng;
use tokio::net::{ToSocketAddrs, TcpStream, TcpListener};
use tokio_stream::wrappers::{TcpListenerStream, ReceiverStream, UnboundedReceiverStream};
use tokio::sync::{mpsc::{self, channel, unbounded_channel, UnboundedSender, UnboundedReceiver, Receiver, Sender}};
use tokio::task::{self, JoinError, JoinHandle};
use tokio::io::{AsyncWriteExt, AsyncWrite};
use tokio_util::sync::{CancellationToken, DropGuard};
use tracing::{instrument, error, debug, info, warn};
use futures::{stream::{Stream, StreamExt, FusedStream}, select, future::{FutureExt, FusedFuture, Fuse}, stream};
use rand::thread_rng;
use tokio::net::tcp::OwnedWriteHalf;
use uuid::Uuid;
use discrete_log_server::algo::{miller_rabin, PollardsLog, PollardsRSAFact};

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
    let mut broker_handle = task::spawn(main_broker(broker_recv, buf_size));
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

    info!("accept loop dropping broker sender, initiating graceful shutdown");
    drop(broker_send);

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
            Frame::RSA { n, e} => Event::RSA { peer_id, n },
            Frame::Prime { p} => Event::Prime { peer_id, p },
            Frame::Quit => {
                // The client is quitting the application, so break
                broker_send.send(Event::Quit { peer_id })
                    .await
                    .map_err(|_e| ServerError::ChannelSend("Client {} unable to send event to main broker".to_string()))?;
                info!(peer_id = ?peer_id, "Client {} read task is exiting loop", peer_id);
                break;
            },
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
#[instrument(ret, err, skip(client_writer, broker_recv, token))]
async fn client_write_task(peer_id: Uuid, client_writer: &mut OwnedWriteHalf, broker_recv: &mut Receiver<Response>, token: CancellationToken) -> Result<(), ServerError> {
    debug!(peer_id = ?peer_id, "inside client write task");
    // Get mutable versions for writing
    let mut client_writer = client_writer;
    // let mut broker_recv = ReceiverStream::new(broker_recv).fuse();
    let mut shutdown_signal = Box::pin(token.cancelled().fuse());

    loop {
        // Select over possible receiving channels
        let response = select! {
            resp = broker_recv.recv().fuse() => {
                match resp {
                    Some(r) => r,
                    None => {
                        // Error state, should not receive none from this receiver
                        error!(peer_id = ?peer_id, "client {} write task received `None` from broker", peer_id);
                        return Err(ServerError::ChannelReceive(format!("client {} write task received `None` from main broker", peer_id)));
                    }
                }
            },
            _ = shutdown_signal => {
                info!(peer_id = ?peer_id, "client {} write task received shutdown signal", peer_id);
                break;
            }
        };

        info!(response = ?response, peer_id = ?peer_id, "client write task received response from main broker");

        match response {
            Response::ConnectionOk => {
                client_writer.write_all(&Response::ConnectionOk.serialize())
                    .await
                    .map_err(|e| ServerError::Write(e))?;
            }
            Response::NotPrime { p } => {
                client_writer.write_all(&Response::NotPrime{ p }.serialize() )
                    .await
                    .map_err(|e| ServerError::Write(e))?;
            }
            Response::Prime { p, prob } => {
                client_writer.write_all(&Response::Prime { p, prob }.serialize())
                    .await
                    .map_err(|e| ServerError::Write(e))?;
            }
            Response::Log { mut pollards } => {
                while let Some(log_item) = StreamExt::next(&mut pollards).await {
                    client_writer.write_all(&Response::LogItem { item: log_item }.serialize())
                        .await
                        .map_err(|e| ServerError::Write(e))?;
                }
                // Check if the discrete log is solvable
                if let Some(log) = pollards.solve() {
                    info!(peer_id = ?peer_id, "discrete logarithm solved successfully");
                    let ratio = pollards.steps_to_sqrt_mod_ratio();
                    client_writer.write_all(&Response::SuccessfulLog { log, g: pollards.g, h: pollards.h, p: pollards.p, ratio }.serialize())
                        .await
                        .map_err(|e| ServerError::Write(e))?;
                } else {
                    info!(peer_id = ?peer_id, "discrete logarithm not solved");
                    // We need to inform the client that solving the logarithm was unsuccessful
                    client_writer.write_all(&Response::UnsuccessfulLog { g: pollards.g, h: pollards.h, p: pollards.p }.serialize())
                        .await
                        .map_err(|e| ServerError::Write(e))?;
                }
            }
            Response::RSA { mut pollards } => {
                while let Some(rsa_item) = StreamExt::next(&mut pollards).await {
                    client_writer.write_all(&Response::RSAItem { item: rsa_item }.serialize())
                        .await
                        .map_err(|e| ServerError::Write(e))?;
                }
                // Check if we were able to factor the public key
                if let Some(p) = pollards.factor() {
                    info!(peer_id = ?peer_id, "public key factored successfully");
                    let q = pollards.n / p;
                    let ratio = pollards.steps_to_sqrt_mod_ratio();
                    client_writer.write_all(&Response::SuccessfulRSA { p, q, ratio }.serialize())
                        .await
                        .map_err(|e| ServerError::Write(e))?;
                } else {
                    info!(peer_id = ?peer_id, "public key not factored successfully");
                    // Otherwise we need to inform client factorization was unsuccessful
                    client_writer.write_all(&Response::UnsuccessfulRSA { n: pollards.n }.serialize())
                        .await
                        .map_err(|e| ServerError::Write(e))?;
                }
            }
            r => return Err(ServerError::IllegalResponse(peer_id, r))
        }
    }

    Ok(())
}

#[instrument(ret, err, skip(events))]
async fn main_broker(events: Receiver<Event>, buf_size: usize) -> Result<(), ServerError> {
    // For mapping from client id's to sending channels
    let mut clients: HashMap<Uuid, Sender<Response>> = HashMap::new();
    // For harvesting disconnected clients
    let (shutdown_send, shutdown_recv) = unbounded_channel::<(Uuid, OwnedWriteHalf, Receiver<Response>)>();

    // Convert to stream and fuse for selecting
    let mut shutdown_recv = UnboundedReceiverStream::new(shutdown_recv).fuse();
    let mut events = ReceiverStream::new(events).fuse();

    // Listen for incoming events
    loop {
        let event = select! {
            // Either we receive an event
            event = events.next().fuse() => {
                match event {
                    Some(ev) => ev,
                    None => {
                        info!("main broker shutting down");
                        break;
                    }
                }
            },
            // Or we harvest a disconnected peer
            (peer_id, client_socket, client_recv) = shutdown_recv.select_next_some().fuse() => {
                info!(peer_id = ?peer_id, "main broker harvesting client {}", peer_id);
                clients.remove(&peer_id).ok_or(ServerError::IllegalState(format!("client with id {} should exist", peer_id)))?;
                continue;
            }
        };

        // Match on the event and generate the correct response
        match event {
            Event::NewClient { peer_id, mut socket, token } => {
                // Create new channel for communicating with new client's write task
                let (client_write_send, mut client_write_recv) = channel::<Response>(buf_size);
                let mut shutdown_send = shutdown_send.clone();
                clients.insert(peer_id, client_write_send.clone());

                task::spawn(async move {
                    let res = client_write_task(peer_id, &mut socket, &mut client_write_recv, token).await;
                    // Client's write task has finished, send signal back to broker
                    if let Err(e) = shutdown_send.send((peer_id, socket, client_write_recv)) {
                        error!(e = ?e, peer_id = ?peer_id,  "error sending shutdown signal to main broker");
                    }
                    if let Err(e) = res {
                        error!(e = ?e, peer_id = ?peer_id, "error from client {} write task", peer_id);
                    }
                });

                // Send the new client a ConnectionOk response
                client_write_send.send(Response::ConnectionOk)
                    .await
                    .map_err(|e| ServerError::ChannelSend(format!("main broker unable to send client {} `ConnectionOk` response after spawning", peer_id)))?;
            }
            Event::Prime { peer_id, p } => {
                // First get the client from the map
                let client_write = clients.get_mut(&peer_id)
                    .ok_or(ServerError::IllegalState(format!("client {} should exist in clients hashmap", peer_id)))?;

                // Run the miller rabin test
                let (prime_flag, prob) = task::spawn_blocking(move || {
                    let mut rng = thread_rng();
                    let mut i = 0;
                    let mut prime_flag = true;
                    while i < 20 {
                        let a = rng.gen_range(2..p);
                        if miller_rabin(p, a) {
                            prime_flag = false;
                            break;
                        }
                        i += 1;
                    }
                    if prime_flag {
                        (prime_flag, 1.0 - f32::powi(0.25, 20))
                    } else {
                        (prime_flag, 0.0)
                    }
                })
                    .await
                    .map_err(|e| ServerError::Task(e))?;

                // Send the correct response accordingly
                if prime_flag {
                    client_write.send(Response::Prime { p, prob })
                        .await
                        .map_err(|e| ServerError::ChannelSend(format!("main broker unable to send `Prime` response to client {} write task", peer_id)))?;
                } else {
                    client_write.send(Response::NotPrime { p })
                        .await
                        .map_err(|e| ServerError::ChannelSend(format!("main broker unable to send `NotPrime` response to client {} write task", peer_id)))?;
                }
            }
            Event::Log { peer_id,  g, h, p } => {
                let mut client_write = clients.get_mut(&peer_id)
                    .ok_or(ServerError::IllegalState(format!("client {} should exist in clients hashmap", peer_id)))?;
                client_write.send(Response::Log { pollards: PollardsLog::new(p, g, h) })
                    .await
                    .map_err(|e| ServerError::ChannelSend(format!("main broker unable to send `Log` response to client {} write task", peer_id)))?;
            }
            Event::RSA { peer_id, n} => {
                let mut client_write = clients.get_mut(&peer_id)
                    .ok_or(ServerError::IllegalState(format!("client {} should exist in clients hashmap", peer_id)))?;
                client_write.send(Response::RSA { pollards: PollardsRSAFact::new(n) })
                    .await
                    .map_err(|e| ServerError::ChannelSend(format!("main broker unable to send `Log` response to client {} write task", peer_id)))?;
            }
            Event::Quit { peer_id } => info!(peer_id = ?peer_id, "main broker received `Quit` event from client {}", peer_id),
        }
    }

    info!("main broker draining shutdown receiver");

    while let Some((peer_id, client_socket, client_recv)) = shutdown_recv.next().await {
        info!(peer_id = ?peer_id, "main broker harvesting client {}", peer_id);
        clients.remove(&peer_id).ok_or(ServerError::IllegalState(format!("client with id {} should exist", peer_id)))?;
    }

    Ok(())
}

#[derive(Debug)]
pub enum ServerError<> {
    Connection(std::io::Error),
    ChannelSend(String),
    ChannelReceive(String),
    IllegalFrame(Uuid, Frame),
    IllegalResponse(Uuid, Response),
    IllegalState(String),
    Read(std::io::Error),
    Task(JoinError),
    Write(std::io::Error),
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::Connection(e) => write!(f, "{:?}", e),
            ServerError::ChannelSend(s) => write!(f, "{s}"),
            ServerError::ChannelReceive(s) => write!(f, "{s}"),
            ServerError::IllegalFrame(id, frame) => write!(f, "illegal frame from client {}: {:?}", id, frame),
            ServerError::IllegalResponse(id, response) => write!(f, "illegal response received by client {}: {:?}", id, response),
            ServerError::IllegalState(s) => write!(f, "{s}"),
            ServerError::Read(e) => write!(f, "{:?}", e),
            ServerError::Task(e) => write!(f, "{:?}", e),
            ServerError::Write(e) => write!(f, "{:?}", e),
        }
    }
}

impl std::error::Error for ServerError {}

fn main() {}


