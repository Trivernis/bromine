use crate::error::Result;
use crate::prelude::IPCResult;
use crate::protocol::{AsyncProtocolStream, AsyncProtocolStreamSplit, AsyncStreamProtocolListener};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::io::Interest;
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::unix::{OwnedReadHalf, SocketAddr};
use tokio::net::{UnixListener, UnixStream};

#[async_trait]
impl AsyncStreamProtocolListener for UnixListener {
    type AddressType = PathBuf;
    type RemoteAddressType = SocketAddr;
    type Stream = UnixStream;

    async fn protocol_bind(address: Self::AddressType) -> Result<Self> {
        let listener = UnixListener::bind(address)?;

        Ok(listener)
    }

    async fn protocol_accept(&self) -> Result<(Self::Stream, Self::RemoteAddressType)> {
        let connection = self.accept().await?;

        Ok(connection)
    }
}

impl AsyncProtocolStreamSplit for UnixStream {
    type OwnedSplitReadHalf = OwnedReadHalf;
    type OwnedSplitWriteHalf = OwnedWriteHalf;

    fn protocol_into_split(self) -> (Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf) {
        self.into_split()
    }
}

#[async_trait]
impl AsyncProtocolStream for UnixStream {
    type AddressType = PathBuf;

    async fn protocol_connect(address: Self::AddressType) -> IPCResult<Self> {
        let stream = UnixStream::connect(address).await?;
        stream
            .ready(Interest::READABLE | Interest::WRITABLE)
            .await?;

        Ok(stream)
    }
}
