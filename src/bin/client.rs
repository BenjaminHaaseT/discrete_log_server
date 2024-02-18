use std::io;
use tokio::io as tokio_io;
use crate::interface::Interface;

mod interface;

pub enum ClientError {
    Response(io::Error),
    Write(io::Error),
    Read(io::Error),
    SendRequest(tokio_io::Error),
    IllegalResponse,
    InterfaceState(Interface)
}

fn main() {
    todo!()
}