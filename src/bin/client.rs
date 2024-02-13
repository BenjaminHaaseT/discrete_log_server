use std::io;
use tokio::io as tokio_io;
mod interface;

pub enum ClientError {
    Response(io::Error),
    Write(io::Error),
    Read(io::Error),
    SendRequest(tokio_io::Error),
}

fn main() {
    todo!()
}