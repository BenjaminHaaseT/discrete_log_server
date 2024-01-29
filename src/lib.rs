use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream, TcpSocket};
use tokio::net::tcp::OwnedWriteHalf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod algo;

use algo::prelude::*;

pub mod prelude {
    pub use super::*;
}

/// An event triggered by a connecting client.
#[derive(Debug)]
pub enum Event {
    /// A new client connecting to the server
    NewClient { peer_id: Uuid, socket: OwnedWriteHalf, token: CancellationToken },

    /// Variant to represent a client request to solve the discrete logarithm
    Log { peer_id: Uuid, g: u64, h: u64, p: u64, },

    /// Variant to represent a client request to find the RSA private key from the given public key
    RSA { peer_id: Uuid, n: u64},

    /// Variant to represent a client request to check if a number is prime or not
    Prime { peer_id: Uuid, p: u64 },

    /// Variant to represent a client disconnecting from the server, mainly for logging
    Quit { peer_id: Uuid }
}

/// A response generated by the server, to be sent back to the client.
pub enum Response {
    /// Represents a successfully established connection
    ConnectionOk,

    /// In case the client sends a number that is not prime
    NotPrime { p: u64 },

    /// Informs client that the number is prime with probability `prob`
    Prime { p: u64, prob: f32 },

    /// For generating the data using Pollards algorithm
    Log { pollards: PollardsLog },

    /// The data for one step of Pollards algorithm
    LogItem { item: PollardsLogItem },

    /// The result of successfully computing the discrete logarithm
    SuccessfulLog { log: u64, g: u64, h: u64, p: u64 },

    /// Informs client that algorithm was unsuccessfully able to determine the discrete log
    UnsuccessfulLog { g: u64, h: u64, p: u64 },

    /// For generating the data using pollards algorithm to factor an RSA key
    RSA { pollards: PollardsRSAFact },

    /// The data generated by completing one step of Pollards algorithm for factoring RSA keys
    RSAItem { item: PollardsRSAFactItem },

    /// Informs the client that the algorithm successfully factored the RSA key
    SuccessfulRSA { p: u64, q: u64 },

    /// Informs the client that the algorithm was unsuccessfully able to factor the RSA key
    UnsuccessfulRSA { n: u64 }
}

impl Response {
    fn serialize_8_bytes(tag: &mut ResponseSerTag, idx: usize, val: u64) {
        for i in 0..8 {
            tag[i + idx] ^= ((val >> (i * 8)) & 0xff) as u8;
        }
    }

    fn deserialize_8_bytes(tag: &ResponseSerTag, idx: usize, val: &mut u64) {
        for i in 0..8 {
            *val ^= (tag[idx + i] as u64) << (i * 8);
        }
    }

    fn serialize_4_bytes(tag: &mut ResponseSerTag, idx: usize, val: u32) {
        for i in 0..4 {
            tag[i + idx] ^= ((val >> (i * 8)) & 0xff) as u8;
        }
    }

    fn deserialize_4_bytes(tag: &ResponseSerTag, idx: usize, val: &mut u32) {
        for i in 0..4 {
            *val ^= (tag[i + idx] as u32) << (i * 8);
        }
    }
}

impl BytesSer for Response {
    type SerTag = ResponseSerTag;

    fn serialize(&self) -> Self::SerTag {
        let mut tag = [0u8; 57];
        match self {
            Response::ConnectionOk => tag[0] ^= 1,
            Response::NotPrime {p} => {
                tag[0] ^= 2;
                Response::serialize_8_bytes(&mut tag, 1, *p);
            }
            Response::Prime {p, prob} => {
                tag[0] ^= 3;
                Response::serialize_8_bytes(&mut tag, 1, *p);
                Response::serialize_4_bytes(&mut tag, 9, (*prob).to_bits())
            }
            Response::LogItem { item} => {
                tag[0] ^= 4;
                Response::serialize_8_bytes(&mut tag, 1, item.i as u64);
                Response::serialize_8_bytes(&mut tag, 9, item.xi);
                Response::serialize_8_bytes(&mut tag, 17, item.ai);
                Response::serialize_8_bytes(&mut tag, 25, item.bi);
                Response::serialize_8_bytes(&mut tag, 33, item.yi);
                Response::serialize_8_bytes(&mut tag, 41, item.gi);
                Response::serialize_8_bytes(&mut tag, 49, item.di);
            }
            Response::SuccessfulLog { log, g, h, p} => {
                tag[0] ^= 5;
                Response::serialize_8_bytes(&mut tag, 1, *log);
                Response::serialize_8_bytes(&mut tag, 9, *g);
                Response::serialize_8_bytes(&mut tag, 17, *h);
                Response::serialize_8_bytes(&mut tag, 25, *p);
            }
            Response::UnsuccessfulLog { g, h, p} => {
                tag[0] ^= 6;
                Response::serialize_8_bytes(&mut tag, 1, *g);
                Response::serialize_8_bytes(&mut tag, 9, *h);
                Response::serialize_8_bytes(&mut tag, 17, *p);
            }
            Response::RSAItem { item} => {
                tag[0] ^= 7;
                Response::serialize_8_bytes(&mut tag, 1, item.i as u64);
                Response::serialize_8_bytes(&mut tag, 9, item.xi);
                Response::serialize_8_bytes(&mut tag, 17, item.yi);
                Response::serialize_8_bytes(&mut tag, 25, item.g);
                Response::serialize_8_bytes(&mut tag, 33, item.n);
            }
            Response::SuccessfulRSA { p, q} => {
                tag[0] ^= 8;
                Response::serialize_8_bytes(&mut tag, 1, *p);
                Response::serialize_8_bytes(&mut tag, 9, *q);
            }
            Response::UnsuccessfulRSA {n} => {
                tag[0] ^= 9;
                Response::serialize_8_bytes(&mut tag, 1, *n);
            }
            _ => panic!("`Response` variant cannot be serialized.")
        }
        tag
    }
}

impl BytesDeser for Response {
    type DeserTag = Response;
    fn deserialize(tag: &Self::SerTag) -> Response {
        match tag[0] {
            1 => Response::ConnectionOk,
            2 => {
                let mut p = 0;
                Response::deserialize_8_bytes(tag, 1, &mut p);
                Response::NotPrime { p }
            }
            3 => {
                let mut p = 0;
                let mut prob = 0;
                Response::deserialize_8_bytes(tag, 1, &mut p);
                Response::deserialize_4_bytes(tag, 9, &mut prob);
                Response::Prime { p, prob: f32::from_bits(prob) }
            }
            4 => {
                let mut i = 0;
                let mut xi = 0;
                let mut ai = 0;
                let mut bi = 0;
                let mut yi = 0;
                let mut gi = 0;
                let mut di = 0;
                Response::deserialize_8_bytes(tag, 1, &mut i);
                Response::deserialize_8_bytes(tag, 9, &mut xi);
                Response::deserialize_8_bytes(tag, 17, &mut ai);
                Response::deserialize_8_bytes(tag, 25, &mut bi);
                Response::deserialize_8_bytes(tag, 33, &mut yi);
                Response::deserialize_8_bytes(tag, 41, &mut gi);
                Response::deserialize_8_bytes(tag, 49, &mut di);
                Response::LogItem { item: PollardsLogItem {
                    i: i as usize,
                    xi,
                    ai,
                    bi,
                    yi,
                    gi,
                    di
                } }
            }
            5 => {
                let mut log = 0;
                let mut g = 0;
                let mut h = 0;
                let mut p = 0;
                Response::deserialize_8_bytes(tag, 1, &mut log);
                Response::deserialize_8_bytes(tag, 9, &mut g);
                Response::deserialize_8_bytes(tag, 17, &mut h);
                Response::deserialize_8_bytes(tag, 25, &mut p);
                Response::SuccessfulLog { log, g, h, p }
            }
            6 => {
                let (mut g, mut h, mut p) = (0, 0, 0);
                Response::deserialize_8_bytes(tag, 1, &mut g);
                Response::deserialize_8_bytes(tag, 9, &mut h);
                Response::deserialize_8_bytes(tag, 17, &mut p);
                Response::UnsuccessfulLog { g, h, p }
            }
            7 => {
                let (mut i, mut xi, mut yi, mut g, mut n) = (0, 0, 0, 0, 0);
                Response::deserialize_8_bytes(tag, 1, &mut i);
                Response::deserialize_8_bytes(tag, 9, &mut xi);
                Response::deserialize_8_bytes(tag, 17, &mut yi);
                Response::deserialize_8_bytes(tag, 25, &mut g);
                Response::deserialize_8_bytes(tag, 33, &mut n);
                Response::RSAItem { item: PollardsRSAFactItem { i: i as usize, xi, yi, g, n }}
            }
            8 => {
                let (mut p, mut q) = (0, 0);
                Response::deserialize_8_bytes(tag, 1, &mut p);
                Response::deserialize_8_bytes(tag, 9, &mut q);
                Response::SuccessfulRSA { p, q }
            }
            9 => {
                let mut n = 0;
                Response::deserialize_8_bytes(tag, 1, &mut n);
                Response::UnsuccessfulRSA { n }
            }
            _ => panic!("Invalid type byte detected when deserializing `Response`")
        }
    }
}

/// The type of serialization tag for a `Response`.
pub type ResponseSerTag = [u8; 57];

impl SerializationTag for ResponseSerTag {}

impl DeserializationTag for Response {}

/// Data that is read from a client's socket
#[derive(Debug, PartialEq)]
pub enum Frame {
    /// A client request to solve the discrete logarithm
    Log { g: u64, h: u64, p: u64 },

    /// A client request to decrypt the RSA private key from the give public key
    RSA { n: u64, e: u64 },

    /// A client request to check if a number is prime or not
    Prime { p: u64 },

    /// A client request to disconnect from the server
    Quit,
}

impl Eq for Frame {}

impl Frame {
    /// Implementation detail of `Frame`, a helper method to aid in serializing into bytes
    fn serialize_8_bytes(tag: &mut [u8; 25], idx: usize, val: u64) {
        for i in 0..8 {
            tag[i + idx] ^= ((val >> (8 * i)) & 0xff) as u8;
        }
    }

    /// Implementation detail of `Frame`, a helper method to aid in deserializing the tag from bytes
    fn deserialize_8_bytes(tag: &[u8; 25], idx: usize, val: &mut u64) {
        for i in 0..8 {
            *val ^= (tag[i + idx] as u64) << (i * 8);
        }
    }

    pub async fn from_reader<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self, std::io::Error> {
        let mut buf = [0u8; 25];
        reader.read_exact(&mut buf).await?;
        Ok(Frame::deserialize(&buf))
    }
}

impl BytesSer for Frame {
    type SerTag = FrameSerTag;

    fn serialize(&self) -> Self::SerTag {
        let mut tag = [0; 25];
        match self {
            Frame::Log { g, h, p } => {
                tag[0] ^= 1;
                Frame::serialize_8_bytes(&mut tag, 1, *g);
                Frame::serialize_8_bytes(&mut tag, 9, *h);
                Frame::serialize_8_bytes(&mut tag, 17, *p);
            },
            Frame::RSA { n, e} => {
                tag[0] ^= 2;
                Frame::serialize_8_bytes(&mut tag, 1, *n);
                Frame::serialize_8_bytes(&mut tag, 9, *e);
            }
            Frame::Prime { p } => {
                tag[0] ^= 3;
                Frame::serialize_8_bytes(&mut tag, 1, *p);
            }
            Frame::Quit => tag[0] ^= 4,
        }
        tag
    }
}

impl BytesDeser for Frame {
    type DeserTag = Frame;

    fn deserialize(tag: &Self::SerTag) -> Self::DeserTag {
        // Bytes 1-3 may represent different pieces of data depending on the variant of self
        let type_byte= tag[0];
        if type_byte ^ 1 == 0 {
            let (mut g, mut h, mut p) = (0u64, 0u64, 0u64);
            Frame::deserialize_8_bytes(&tag, 1, &mut g);
            Frame::deserialize_8_bytes(&tag, 9, &mut h);
            Frame::deserialize_8_bytes(&tag, 17, &mut p);
            Frame::Log { g, h, p}
        } else if type_byte ^ 2 == 0 {
            let (mut n, mut e) = (0u64, 0u64);
            Frame::deserialize_8_bytes(&tag, 1, &mut n);
            Frame::deserialize_8_bytes(&tag, 9, &mut e);
            Frame::RSA { n, e }
        } else if type_byte ^ 3 == 0 {
            let mut p = 0;
            Frame::deserialize_8_bytes(tag, 1, &mut p);
            Frame::Prime { p }
        } else if type_byte ^ 4 == 0 {
            Frame::Quit
        } else {
            panic!("invalid type byte detected when deserializing `Frame`.");
        }
    }
}

impl AsBytes for Frame {
    fn as_bytes(&self) -> Vec<u8> {
        self.serialize().to_vec()
    }
}

/// The serialization tag for `Frame`
///
/// One byte for the type and up to 24 bytes for the transmitted data.
pub type FrameSerTag = [u8; 25];

impl SerializationTag for FrameSerTag {}

impl DeserializationTag for Frame {}

/// An interface for any type that can be serialized into bytes.
pub trait BytesSer {
    /// Associated type for the tag `self` will serialize as.
    type SerTag: SerializationTag;

    /// Required method,
    /// takes a reference to `self` and returns a `Self::Tag`.
    fn serialize(&self) -> Self::SerTag;
}

/// An interface for any type that can be deserialized from bytes.
pub trait BytesDeser: BytesSer {
    /// Associated type for the tag that `Self::SerTag` will deserialize as.
    type DeserTag: DeserializationTag;

    /// Required method,
    /// takes a reference to `Self::SerTag` and returns a `Self::DeSerTag`
    fn deserialize(tag: &Self::SerTag) -> Self::DeserTag;
}

/// Marker trait. Intended to be implemented by any type that is a `SerTag`.
pub trait SerializationTag {}

/// Marker trait. Intended to be implemented by any type that is a `DeSerTag`.
pub trait DeserializationTag {}

/// An interface for any type that can be serialized into bytes and deserialized from bytes
pub trait AsBytes: BytesDeser {
    /// Required method, takes a `self` shared reference and returns the byte representation
    fn as_bytes(&self) -> Vec<u8>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_frame_should_work() {
        // let frame = Frame::Connect;
        // let tag = frame.serialize();
        // println!("{:?}", tag);
        // assert_eq!(tag, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let frame = Frame::Log { g: 3, h: 2, p: 7 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [1, 3, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0]);

        let frame = Frame::Log { g: 627, h: 390, p: 941 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [1, 115, 2, 0, 0, 0, 0, 0, 0, 134, 1, 0, 0, 0, 0, 0, 0, 173, 3, 0, 0, 0, 0, 0, 0]);

        let frame = Frame::RSA { n: 1794677960, e: 525734818};
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [2, 200, 156, 248, 106, 0, 0, 0, 0, 162, 19, 86, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let frame = Frame::RSA { n: 38749709, e: 10988423 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [2, 13, 70, 79, 2, 0, 0, 0, 0, 135, 171, 167, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let frame = Frame::Prime { p: 15239131 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [3, 219, 135, 232, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let frame = Frame::Quit;
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn deserialize_frame_should_work() {
        let frame = Frame::Log { g: 3, h: 2, p: 7 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [1, 3, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0]);

        let deserialized_frame = Frame::deserialize(&tag);
        println!("{:?}", deserialized_frame);
        assert_eq!(deserialized_frame, frame);

        let frame = Frame::Log { g: 627, h: 390, p: 941 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [1, 115, 2, 0, 0, 0, 0, 0, 0, 134, 1, 0, 0, 0, 0, 0, 0, 173, 3, 0, 0, 0, 0, 0, 0]);

        let deserialized_frame = Frame::deserialize(&tag);
        println!("{:?}", deserialized_frame);
        assert_eq!(deserialized_frame, frame);

        let frame = Frame::RSA { n: 1794677960, e: 525734818};
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [2, 200, 156, 248, 106, 0, 0, 0, 0, 162, 19, 86, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let deserialized_frame = Frame::deserialize(&tag);
        println!("{:?}", deserialized_frame);
        assert_eq!(deserialized_frame, frame);

        let frame = Frame::RSA { n: 38749709, e: 10988423 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [2, 13, 70, 79, 2, 0, 0, 0, 0, 135, 171, 167, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let deserialized_frame = Frame::deserialize(&tag);
        println!("{:?}", deserialized_frame);
        assert_eq!(deserialized_frame, frame);

        let frame = Frame::Prime { p: 15239131 };
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [3, 219, 135, 232, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let deserialized_frame = Frame::deserialize(&tag);
        println!("{:?}", deserialized_frame);
        assert_eq!(frame, deserialized_frame);

        let frame = Frame::Quit;
        let tag = frame.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let deserialized_frame = Frame::deserialize(&tag);
        println!("{:?}", deserialized_frame);
        assert_eq!(deserialized_frame, frame);
    }

    #[test]
    fn serialize_response_should_work() {
        let response = Response::ConnectionOk;
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let response = Response::NotPrime { p: 8 };
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [2, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);


        let response = Response::Prime { p: 31, prob: 0.9942 };
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [3, 31, 0, 0, 0, 0, 0, 0, 0, 228, 131, 126, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let response = Response::LogItem { item: PollardsLogItem { i: 3, xi: 127, yi: 64, ai: 128, bi: 32, gi: 55, di: 89}};
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [4, 3, 0, 0, 0, 0, 0, 0, 0, 127, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 55, 0, 0, 0, 0, 0, 0, 0, 89, 0, 0, 0, 0, 0, 0, 0]);

        let response = Response::SuccessfulLog { log: 11, g: 2, h: 63, p: 71 };
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [5, 11, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let response = Response::UnsuccessfulLog { g: 2, h: 63, p: 71 };
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [6, 2, 0, 0, 0, 0, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 71, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,]);

        let response = Response::RSAItem { item: PollardsRSAFactItem { i: 1, xi: 2, yi: 3, g: 1, n: 15}};
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [7, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let response = Response::SuccessfulRSA { p: 3, q: 5 };
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [8, 3, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let response = Response::UnsuccessfulRSA { n: 15 };
        let tag = response.serialize();
        println!("{:?}", tag);
        assert_eq!(tag, [9, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
}

