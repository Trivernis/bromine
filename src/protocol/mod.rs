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

    async fn bind(address: Self::AddressType) -> IPCResult<Self>;

    async fn accept(&self) -> IPCResult<(Self::Stream, Self::RemoteAddressType)>;
}

#[async_trait]
pub trait AsyncProtocolStream: AsyncRead + AsyncWrite + Sized + Send + Sync {
    type AddressType;
    type OwnedSplitReadHalf: AsyncRead + Send + Sync;
    type OwnedSplitWriteHalf: AsyncWrite + Send + Sync;

    async fn connect(address: Self::AddressType) -> IPCResult<Self>;

    async fn into_split(self) -> IPCResult<(Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf)>;
}
