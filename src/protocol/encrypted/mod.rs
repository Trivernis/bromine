mod crypt_handling;
mod io_impl;
mod protocol_impl;

use bytes::{BufMut, Bytes, BytesMut};
pub use io_impl::*;
pub use protocol_impl::*;
use rand::RngCore;
use std::future::Future;
use std::io;
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use x25519_dalek::{SharedSecret, StaticSecret};

use crate::prelude::encrypted::crypt_handling::CipherBox;
use crate::prelude::{AsyncProtocolStream, AsyncStreamProtocolListener};

pub type OptionalFuture<T> = Option<Pin<Box<dyn Future<Output = T> + Send + Sync>>>;

#[derive(Clone)]
pub struct EncryptionOptions<T: Clone + Default> {
    pub inner_options: T,
    pub secret: StaticSecret,
}

impl<T: Clone + Default> Default for EncryptionOptions<T> {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let mut secret = [0u8; 32];
        rng.fill_bytes(&mut secret);

        Self {
            secret: StaticSecret::from(secret),
            inner_options: T::default(),
        }
    }
}

pub struct EncryptedListener<T: AsyncStreamProtocolListener> {
    inner: T,
    secret: StaticSecret,
}

impl<T: AsyncStreamProtocolListener> EncryptedListener<T> {
    pub fn new(inner: T, secret: StaticSecret) -> Self {
        Self { inner, secret }
    }
}

pub struct EncryptedStream<T: AsyncProtocolStream> {
    read_half: EncryptedReadStream<T::OwnedSplitReadHalf>,
    write_half: EncryptedWriteStream<T::OwnedSplitWriteHalf>,
}

impl<T: AsyncProtocolStream> EncryptedStream<T> {
    pub fn new(inner: T, secret: SharedSecret) -> Self {
        let cipher_box = CipherBox::new(Bytes::from(secret.to_bytes().to_vec()));
        let (read, write) = inner.protocol_into_split();
        let read_half = EncryptedReadStream::new(read, cipher_box.clone());
        let write_half = EncryptedWriteStream::new(write, cipher_box);

        Self {
            read_half,
            write_half,
        }
    }

    pub fn update_key(&mut self, key: Bytes) {
        self.write_half
            .cipher
            .as_mut()
            .unwrap()
            .update_key(key.clone());
        self.read_half
            .cipher
            .as_mut()
            .unwrap()
            .update_key(key.clone());
    }
}

pub struct EncryptedReadStream<T: AsyncRead> {
    inner: Option<T>,
    fut: OptionalFuture<(io::Result<Bytes>, T, CipherBox)>,
    remaining: BytesMut,
    cipher: Option<CipherBox>,
}

impl<T: 'static + AsyncRead + Unpin + Send + Sync> EncryptedReadStream<T> {
    pub(crate) fn new(inner: T, cipher: CipherBox) -> Self {
        Self {
            inner: Some(inner),
            fut: None,
            remaining: BytesMut::new(),
            cipher: Some(cipher),
        }
    }
}

pub struct EncryptedWriteStream<T: 'static + AsyncWrite + Unpin + Send + Sync> {
    inner: Option<T>,
    cipher: Option<CipherBox>,
    buffer: BytesMut,
    fut_write: OptionalFuture<(io::Result<()>, T, CipherBox)>,
    fut_flush: OptionalFuture<(io::Result<()>, T, CipherBox)>,
    fut_shutdown: OptionalFuture<io::Result<()>>,
}

impl<T: 'static + AsyncWrite + Unpin + Send + Sync> EncryptedWriteStream<T> {
    pub(crate) fn new(inner: T, cipher: CipherBox) -> Self {
        Self {
            inner: Some(inner),
            cipher: Some(cipher),
            buffer: BytesMut::with_capacity(1024),
            fut_write: None,
            fut_flush: None,
            fut_shutdown: None,
        }
    }
}

pub(crate) struct EncryptedPackage {
    bytes: Bytes,
}

impl EncryptedPackage {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn into_bytes(self) -> Bytes {
        let mut buf = BytesMut::with_capacity(4 + self.bytes.len());
        buf.put_u32(self.bytes.len() as u32);
        buf.put(self.bytes);

        buf.freeze()
    }

    pub async fn from_async_read<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Self> {
        let length = reader.read_u32().await?;
        let mut bytes_buf = vec![0u8; length as usize];
        reader.read_exact(&mut bytes_buf).await?;

        Ok(Self {
            bytes: Bytes::from(bytes_buf),
        })
    }

    pub fn into_inner(self) -> Bytes {
        self.bytes
    }
}
