use async_trait::async_trait;
use bromine::error::Result;
use bromine::prelude::{AsyncProtocolStreamSplit, IPCError};
use bromine::protocol::{AsyncProtocolStream, AsyncStreamProtocolListener};
use lazy_static::lazy_static;
use std::cmp::min;
use std::collections::HashMap;
use std::future::Future;
use std::io::Error;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;

lazy_static! {
    static ref LISTENERS_REF: Arc<Mutex<HashMap<u8, Sender<TestProtocolStream>>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

/// Adds a channel that receives streams to handle
async fn add_port(number: u8, sender: tokio::sync::mpsc::Sender<TestProtocolStream>) {
    let mut listeners = LISTENERS_REF.lock().await;
    listeners.insert(number, sender);
}

/// Returns a stream for the given port connecting with the server via channels
async fn get_port(number: u8) -> Option<TestProtocolStream> {
    let mut listeners = LISTENERS_REF.lock().await;

    if let Some(sender) = listeners.get_mut(&number) {
        let (s1, r1) = channel(2);
        let (s2, r2) = channel(2);
        let stream_1 = TestProtocolStream {
            sender: s1,
            receiver: Arc::new(Mutex::new(r2)),
            future: None,
            remaining_buf: Default::default(),
        };
        let stream_2 = TestProtocolStream {
            sender: s2,
            receiver: Arc::new(Mutex::new(r1)),
            future: None,
            remaining_buf: Default::default(),
        };
        sender.send(stream_2).await.ok();

        Some(stream_1)
    } else {
        None
    }
}

pub struct TestProtocolListener {
    receiver: Arc<Mutex<Receiver<TestProtocolStream>>>,
}

#[async_trait]
impl AsyncStreamProtocolListener for TestProtocolListener {
    type AddressType = u8;
    type RemoteAddressType = u8;
    type Stream = TestProtocolStream;

    async fn protocol_bind(address: Self::AddressType) -> Result<Self> {
        let (sender, receiver) = channel(1);
        add_port(address, sender).await;

        Ok(Self {
            receiver: Arc::new(Mutex::new(receiver)),
        })
    }

    async fn protocol_accept(&self) -> Result<(Self::Stream, Self::RemoteAddressType)> {
        self.receiver
            .lock()
            .await
            .recv()
            .await
            .map(|r| (r, 0u8))
            .ok_or_else(|| IPCError::from("Failed to accept"))
    }
}

impl Clone for TestProtocolStream {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            receiver: Arc::clone(&self.receiver),
            future: None,
            remaining_buf: Default::default(),
        }
    }
}

pub struct TestProtocolStream {
    sender: Sender<Vec<u8>>,
    receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
    future: Option<Pin<Box<dyn Future<Output = ()> + Send + Sync>>>,
    remaining_buf: Arc<Mutex<Vec<u8>>>,
}

impl TestProtocolStream {
    /// Read from the receiver and remaining buffer
    async fn read_from_receiver(
        buf: &mut ReadBuf<'static>,
        receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
        remaining_buf: Arc<Mutex<Vec<u8>>>,
    ) {
        {
            let mut remaining_buf = remaining_buf.lock().await;
            if !remaining_buf.is_empty() {
                if Self::read_from_remaining_buffer(buf, &mut remaining_buf).await {
                    return;
                }
            }
        }
        let mut receiver = receiver.lock().await;

        if let Some(mut bytes) = receiver.recv().await {
            let slice_len = min(bytes.len(), buf.capacity());

            buf.put_slice(&bytes[0..slice_len]);
            bytes.reverse();
            bytes.truncate(bytes.len() - slice_len);
            bytes.reverse();
            let mut remaining_buf = remaining_buf.lock().await;
            remaining_buf.append(&mut bytes);
        }
    }

    /// Read from the remaining buffer returning a boolean if the
    /// read buffer has been filled
    async fn read_from_remaining_buffer(
        buf: &mut ReadBuf<'static>,
        remaining_buf: &mut Vec<u8>,
    ) -> bool {
        if remaining_buf.len() < buf.capacity() {
            buf.put_slice(&remaining_buf);
            remaining_buf.clear();

            false
        } else if remaining_buf.len() == buf.capacity() {
            buf.put_slice(&remaining_buf);
            remaining_buf.clear();

            true
        } else {
            let slice_len = buf.capacity();
            let remaining_len = remaining_buf.len();
            buf.put_slice(&remaining_buf[0..slice_len]);
            remaining_buf.reverse();
            remaining_buf.truncate(remaining_len - slice_len);
            remaining_buf.reverse();
            true
        }
    }
}

impl AsyncProtocolStreamSplit for TestProtocolStream {
    type OwnedSplitReadHalf = Self;
    type OwnedSplitWriteHalf = Self;

    fn protocol_into_split(self) -> (Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf) {
        (self.clone(), self)
    }
}

#[async_trait]
impl AsyncProtocolStream for TestProtocolStream {
    type AddressType = u8;

    async fn protocol_connect(address: Self::AddressType) -> Result<Self> {
        get_port(address)
            .await
            .ok_or_else(|| IPCError::from("Failed to connect"))
    }
}

impl AsyncRead for TestProtocolStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        unsafe {
            // we need a mutable reference to access the inner future
            let stream = self.get_unchecked_mut();

            if stream.future.is_none() {
                // we need to change the lifetime to be able to use the read buffer in the read future
                let buf: &mut ReadBuf<'static> = mem::transmute(buf);
                let receiver = Arc::clone(&stream.receiver);
                let remaining_buf = Arc::clone(&stream.remaining_buf);

                let future = TestProtocolStream::read_from_receiver(buf, receiver, remaining_buf);
                stream.future = Some(Box::pin(future));
            }
            if let Some(future) = &mut stream.future {
                match future.as_mut().poll(cx) {
                    Poll::Ready(_) => {
                        stream.future = None;
                        Poll::Ready(Ok(()))
                    }
                    Poll::Pending => Poll::Pending,
                }
            } else {
                Poll::Pending
            }
        }
    }
}

impl AsyncWrite for TestProtocolStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::prelude::rust_2015::Result<usize, Error>> {
        let write_len = buf.len();
        unsafe {
            // we need a mutable reference to access the inner future
            let stream = self.get_unchecked_mut();

            if stream.future.is_none() {
                // we take ownership here so that we don't need to change lifetimes here
                let buf = buf.to_vec();
                let sender = stream.sender.clone();

                let future = async move {
                    sender.send(buf).await.unwrap();
                };
                stream.future = Some(Box::pin(future));
            }
            if let Some(future) = &mut stream.future {
                match future.as_mut().poll(cx) {
                    Poll::Ready(_) => {
                        stream.future = None;
                        Poll::Ready(Ok(write_len))
                    }
                    Poll::Pending => Poll::Pending,
                }
            } else {
                Poll::Pending
            }
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<std::prelude::rust_2015::Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<std::prelude::rust_2015::Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}
