use crate::prelude::IPCResult;
use crate::protocol::{AsyncProtocolStream, AsyncStreamProtocol, AsyncStreamProtocolListener};
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};

pub struct TcpProtocol;

#[async_trait]
impl AsyncStreamProtocol for TcpProtocol {
    type Listener = TcpListener;
    type Stream = TcpStream;
}

#[async_trait]
impl AsyncStreamProtocolListener for TcpListener {
    type AddressType = SocketAddr;
    type RemoteAddressType = SocketAddr;
    type Stream = TcpStream;

    async fn protocol_bind(address: Self::AddressType) -> IPCResult<Self> {
        let listener = TcpListener::bind(address).await?;

        Ok(listener)
    }

    async fn protocol_accept(&self) -> IPCResult<(Self::Stream, Self::RemoteAddressType)> {
        let connection = self.accept().await?;

        Ok(connection)
    }
}

#[async_trait]
impl AsyncProtocolStream for TcpStream {
    type AddressType = SocketAddr;
    type OwnedSplitReadHalf = OwnedReadHalf;
    type OwnedSplitWriteHalf = OwnedWriteHalf;

    async fn protocol_connect(address: Self::AddressType) -> IPCResult<Self> {
        let stream = TcpStream::connect(address).await?;

        Ok(stream)
    }

    async fn protocol_into_split(self) -> (Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf) {
        self.into_split()
    }
}
