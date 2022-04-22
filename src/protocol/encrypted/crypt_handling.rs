use crate::prelude::encrypted::{EncryptedStream, Keys};
use crate::prelude::{IPCError, IPCResult};
use crate::protocol::AsyncProtocolStream;
use bytes::Bytes;
use chacha20poly1305::aead::{Aead, NewAead};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::thread_rng;
use rand_core::RngCore;
use sha2::{Digest, Sha256};
use std::io;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use x25519_dalek::{PublicKey, StaticSecret};

/// A structure used for encryption.
/// It holds the cipher initialized with the given key
/// and two counters for both encryption and decryption
/// count which are used to keep track of the nonce.
#[derive(Clone)]
pub(crate) struct CipherBox {
    cipher: ChaCha20Poly1305,
    en_count: Arc<AtomicU64>,
    de_count: Arc<AtomicU64>,
}

impl CipherBox {
    pub fn new(key: Bytes) -> Self {
        let key = Key::from_slice(&key[..]);
        let cipher = ChaCha20Poly1305::new(key);

        Self {
            cipher,
            en_count: Arc::new(AtomicU64::new(0)),
            de_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Encrypts the given message
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn encrypt(&self, data: Bytes) -> io::Result<Bytes> {
        self.cipher
            .encrypt(&self.en_nonce(), &data[..])
            .map(Bytes::from)
            .map_err(|_| io::Error::from(ErrorKind::InvalidData))
    }

    /// Decrypts the given message
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn decrypt(&self, data: Bytes) -> io::Result<Bytes> {
        self.cipher
            .decrypt(&self.de_nonce(), &data[..])
            .map(Bytes::from)
            .map_err(|_| io::Error::from(ErrorKind::InvalidData))
    }

    /// Updates the stored key.
    /// This must be done simultaneously on server and client side
    /// to keep track of the nonce
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn update_key(&mut self, key: Bytes) {
        let key = Key::from_slice(&key[..]);
        self.cipher = ChaCha20Poly1305::new(key);
        self.reset_counters();
    }

    /// Resets the nonce counters.
    /// This must be done simultaneously on server and client side.
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn reset_counters(&mut self) {
        self.de_count.store(0, Ordering::SeqCst);
        self.en_count.store(0, Ordering::SeqCst);
    }

    fn en_nonce(&self) -> Nonce {
        let count = self.en_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("encrypted count {}", count);
        nonce_from_number(count)
    }

    fn de_nonce(&self) -> Nonce {
        let count = self.de_count.fetch_add(1, Ordering::SeqCst);
        tracing::trace!("decrypted count {}", count);
        nonce_from_number(count)
    }
}

/// Generates a nonce from a given number
/// The given number is repeated to fit the nonce bytes
fn nonce_from_number(number: u64) -> Nonce {
    let number_bytes: [u8; 8] = number.to_be_bytes();
    let num_vec = number_bytes.repeat(2);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&num_vec[..12]);

    nonce_bytes.into()
}

impl<T: AsyncProtocolStream> EncryptedStream<T> {
    /// Does a server-client key exchange.
    /// 1. The server receives the public key of the client
    /// 2. The server sends its own public key
    /// 3. The server creates an intermediary encrypted connection
    /// 4. The server generates a new secret
    /// 5. The server sends the secret to the client
    /// 6. The connection is upgraded with the new shared key
    pub async fn from_server_key_exchange(mut inner: T, keys: &Keys) -> IPCResult<Self> {
        let other_pub = receive_public_key(&mut inner).await?;
        tracing::debug!("received peer public key {:?}", other_pub);

        if !keys.allow_unknown && !keys.known_peers.contains(&other_pub) {
            return Err(IPCError::UnknownPeer(other_pub));
        }
        send_public_key(&mut inner, &keys.secret).await?;
        let shared_secret = keys.secret.diffie_hellman(&other_pub);
        let mut stream = Self::new(inner, shared_secret);
        let permanent_secret = generate_secret();
        stream.write_all(&permanent_secret).await?;
        stream.flush().await?;
        stream.update_key(permanent_secret.into());
        tracing::debug!("Connection established");

        Ok(stream)
    }

    /// Does a client-server key exchange.
    /// 1. The client sends its public key to the server
    /// 2. The client receives the servers public key
    /// 3. The client creates an intermediary encrypted connection
    /// 4. The client receives the new key from the server
    /// 5. The connection is upgraded with the new shared key
    pub async fn from_client_key_exchange(mut inner: T, keys: &Keys) -> IPCResult<Self> {
        send_public_key(&mut inner, &keys.secret).await?;
        let other_pub = receive_public_key(&mut inner).await?;
        tracing::debug!("received peer public key {:?}", other_pub);

        if !keys.allow_unknown && !keys.known_peers.contains(&other_pub) {
            return Err(IPCError::UnknownPeer(other_pub));
        }
        let shared_secret = keys.secret.diffie_hellman(&other_pub);
        let mut stream = Self::new(inner, shared_secret);
        let mut key_buf = vec![0u8; 32];
        stream.read_exact(&mut key_buf).await?;
        stream.update_key(key_buf.into());
        tracing::debug!("Connection established");

        Ok(stream)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn receive_public_key<T: AsyncProtocolStream>(stream: &mut T) -> IPCResult<PublicKey> {
    let mut pk_buf = [0u8; 32];
    stream.read_exact(&mut pk_buf).await?;

    Ok(PublicKey::from(pk_buf))
}

#[tracing::instrument(level = "debug", skip_all)]
async fn send_public_key<T: AsyncProtocolStream>(
    stream: &mut T,
    secret: &StaticSecret,
) -> IPCResult<()> {
    let own_pk = PublicKey::from(secret);
    stream.write_all(own_pk.as_bytes()).await?;
    stream.flush().await?;

    Ok(())
}

#[tracing::instrument(level = "trace", skip_all)]
fn generate_secret() -> Vec<u8> {
    let mut rng = thread_rng();
    let mut buf = vec![0u8; 32];
    rng.fill_bytes(&mut buf);

    Sha256::digest(&buf).to_vec()
}
