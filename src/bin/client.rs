use std::io;
mod interface;

pub enum ClientError {
    Response(io::Error),
    Write(io::Error),
}

fn main() {
    todo!()
}