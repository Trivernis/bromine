mod tcp;

use crate::prelude::IPCResult;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait AsyncStreamProtocol {
    type Listener: AsyncStreamProtocolListener;
    type Stream: AsyncProtocolStream;
}

#[async_trait]
pub trait AsyncStreamProtocolListener: Sized {
    type AddressType;
    type RemoteAddressType;
    type Stream: AsyncProtocolStream;

    async fn protocol_bind(address: Self::AddressType) -> IPCResult<Self>;

    async fn protocol_accept(&self) -> IPCResult<(Self::Stream, Self::RemoteAddressType)>;
}

#[async_trait]
pub trait AsyncProtocolStream: AsyncRead + AsyncWrite + Sized + Send + Sync {
    type AddressType;
    type OwnedSplitReadHalf: AsyncRead + Send + Sync;
    type OwnedSplitWriteHalf: AsyncWrite + Send + Sync;

    async fn protocol_connect(address: Self::AddressType) -> IPCResult<Self>;

    async fn protocol_into_split(self) -> (Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf);
}
