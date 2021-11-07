pub mod tcp;

use crate::prelude::IPCResult;
use async_trait::async_trait;
use std::fmt::Debug;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait AsyncStreamProtocolListener: Sized {
    type AddressType: ToString + Clone + Debug;
    type RemoteAddressType: ToString;
    type Stream: 'static + AsyncProtocolStream<AddressType = Self::AddressType>;

    async fn protocol_bind(address: Self::AddressType) -> IPCResult<Self>;

    async fn protocol_accept(&self) -> IPCResult<(Self::Stream, Self::RemoteAddressType)>;
}

pub trait AsyncProtocolStreamSplit {
    type OwnedSplitReadHalf: AsyncRead + Send + Sync + Unpin;
    type OwnedSplitWriteHalf: AsyncWrite + Send + Sync + Unpin;

    fn protocol_into_split(self) -> (Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf);
}

#[async_trait]
pub trait AsyncProtocolStream:
    AsyncRead + AsyncWrite + Sized + Send + Sync + AsyncProtocolStreamSplit
{
    type AddressType: ToString + Clone + Debug;

    async fn protocol_connect(address: Self::AddressType) -> IPCResult<Self>;
}
