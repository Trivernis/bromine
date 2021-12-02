use async_trait::async_trait;
use bromine::error::Result;
use bromine::prelude::{AsyncProtocolStreamSplit, IPCError};
use bromine::protocol::{AsyncProtocolStream, AsyncStreamProtocolListener};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::io::Error;
use std::pin::Pin;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc::{
    channel as async_channel, Receiver as AsyncReceiver, Sender as AsyncSender,
};
use tokio::sync::Mutex as AsyncMutex;

lazy_static! {
    static ref LISTENERS_REF: Arc<AsyncMutex<HashMap<u8, AsyncSender<TestProtocolStream>>>> =
        Arc::new(AsyncMutex::new(HashMap::new()));
}

async fn add_port(number: u8, sender: tokio::sync::mpsc::Sender<TestProtocolStream>) {
    let mut listeners = LISTENERS_REF.lock().await;
    listeners.insert(number, sender);
}

async fn get_port(number: u8) -> Option<TestProtocolStream> {
    let mut listeners = LISTENERS_REF.lock().await;

    if let Some(sender) = listeners.get_mut(&number) {
        let (s1, r1) = mpsc::channel();
        let (s2, r2) = mpsc::channel();
        let stream_1 = TestProtocolStream {
            sender: Arc::new(Mutex::new(s1)),
            receiver: Arc::new(Mutex::new(r2)),
        };
        let stream_2 = TestProtocolStream {
            sender: Arc::new(Mutex::new(s2)),
            receiver: Arc::new(Mutex::new(r1)),
        };
        sender.send(stream_2).await.ok();

        Some(stream_1)
    } else {
        None
    }
}

pub struct TestProtocolListener {
    receiver: Arc<AsyncMutex<AsyncReceiver<TestProtocolStream>>>,
}

#[async_trait]
impl AsyncStreamProtocolListener for TestProtocolListener {
    type AddressType = u8;
    type RemoteAddressType = u8;
    type Stream = TestProtocolStream;

    async fn protocol_bind(address: Self::AddressType) -> Result<Self> {
        let (sender, receiver) = async_channel(1);
        add_port(address, sender).await;

        Ok(Self {
            receiver: Arc::new(AsyncMutex::new(receiver)),
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

#[derive(Clone)]
pub struct TestProtocolStream {
    sender: Arc<Mutex<Sender<Vec<u8>>>>,
    receiver: Arc<Mutex<Receiver<Vec<u8>>>>,
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
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let receiver = self.receiver.lock().unwrap();
        if let Ok(b) = receiver.recv() {
            buf.put_slice(&b);
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl AsyncWrite for TestProtocolStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::prelude::rust_2015::Result<usize, Error>> {
        let sender = self.sender.lock().unwrap();
        let vec_buf = buf.to_vec();
        let buf_len = vec_buf.len();
        sender.send(vec_buf).unwrap();

        Poll::Ready(Ok(buf_len))
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
