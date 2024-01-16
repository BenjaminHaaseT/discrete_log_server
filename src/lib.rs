use std::sync::Arc;
use tokio::net::{TcpStream, TcpSocket};
use uuid::Uuid;

/// An event triggered by a connecting client.
pub enum Event {
    /// A new client connecting to the server
    NewClient { peer_id: Uuid, socket: Arc<TcpStream> },

    /// Variant to represent a client request to solve the discrete logarithm
    Log { peer_id: Uuid, g: u64, h: u64, p: u64, },

    /// Variant to represent a client request to find the RSA private key from the given public key
    RSA { peer_id: Uuid, n: u64, e: u64},

    /// Variant to represent a client disconnecting from the server, mainly for logging
    Quit { peer_id: Uuid }
}

/// Data that is read from a client's socket
pub enum Frame {
    /// A client request to connect to the server
    Connect,

    /// A client request to solve the discrete logarithm
    Log { g: u64, h: u64, p: u64 },

    /// A client request to decrypt the RSA private key from the give public key
    RSA { n: u64, e: u64 },

    /// A client request to disconnect from the server
    Quit,
}

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
}

impl BytesSer for Frame {
    type SerTag = FrameSerTag;

    fn serialize(&self) -> Self::SerTag {
        let mut tag = [0; 25];
        match self {
            Frame::Connect => tag[0] ^= 1,
            Frame::Log { g, h, p } => {
                tag[0] ^= 2;
                Frame::serialize_8_bytes(&mut tag, 1, *g);
                Frame::serialize_8_bytes(&mut tag, 9, *h);
                Frame::serialize_8_bytes(&mut tag, 17, *p);
            },
            Frame::RSA { n, e} => {
                tag[0] ^= 3;
                Frame::serialize_8_bytes(&mut tag, 1, *n);
                Frame::serialize_8_bytes(&mut tag, 9, *e);
            }
            Frame::Quit => tag[0] ^= 4
        }
        tag
    }
}

impl BytesDeser for Frame {
    type DeserTag = Frame;

    fn deserialize(tag: &Self::SerTag) -> Self::DeserTag {
        // Bytes 1-3 may represent different pieces of data depending on the variant of self
        let type_byte= tag[0];
        if type_byte ^ 1 == 0  {
            Frame::Connect
        } else if type_byte ^ 2 == 0 {
            let (mut g, mut h, mut p) = (0u64, 0u64, 0u64);
            Frame::deserialize_8_bytes(&tag, 1, &mut g);
            Frame::deserialize_8_bytes(&tag, 9, &mut h);
            Frame::deserialize_8_bytes(&tag, 17, &mut p);
            Frame::Log { g, h, p}
        } else if type_byte ^ 3 == 0 {
            let (mut n, mut e) = (0u64, 0u64);
            Frame::deserialize_8_bytes(&tag, 1, &mut n);
            Frame::deserialize_8_bytes(&tag, 9, &mut e);
            Frame::RSA { n, e }
        } else if type_byte ^ 4 == 0 {
            Frame::Quit
        } else  {
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



