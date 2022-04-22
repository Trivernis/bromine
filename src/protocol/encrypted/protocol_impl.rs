use crate::error::Result;
use crate::prelude::encrypted::{EncryptedReadStream, EncryptedWriteStream};
use crate::prelude::{AsyncProtocolStreamSplit, IPCResult};
use crate::protocol::encrypted::{EncryptedListener, EncryptedStream, EncryptionOptions};
use crate::protocol::{AsyncProtocolStream, AsyncStreamProtocolListener};
use async_trait::async_trait;

#[async_trait]
impl<T: AsyncStreamProtocolListener> AsyncStreamProtocolListener for EncryptedListener<T> {
    type AddressType = T::AddressType;
    type RemoteAddressType = T::RemoteAddressType;
    type Stream = EncryptedStream<T::Stream>;
    type ListenerOptions = EncryptionOptions<T::ListenerOptions>;

    async fn protocol_bind(
        address: Self::AddressType,
        options: Self::ListenerOptions,
    ) -> IPCResult<Self> {
        let inner = T::protocol_bind(address, options.inner_options).await?;

        Ok(EncryptedListener::new(inner, options.keys))
    }

    async fn protocol_accept(&self) -> IPCResult<(Self::Stream, Self::RemoteAddressType)> {
        let (inner_stream, remote_addr) = self.inner.protocol_accept().await?;
        let stream = Self::Stream::from_server_key_exchange(inner_stream, &self.keys).await?;

        Ok((stream, remote_addr))
    }
}

#[async_trait]
impl<T: AsyncProtocolStream> AsyncProtocolStream for EncryptedStream<T> {
    type AddressType = T::AddressType;
    type StreamOptions = EncryptionOptions<T::StreamOptions>;

    async fn protocol_connect(
        address: Self::AddressType,
        options: Self::StreamOptions,
    ) -> Result<Self> {
        let inner = T::protocol_connect(address, options.inner_options).await?;
        EncryptedStream::from_client_key_exchange(inner, &options.keys).await
    }
}

#[async_trait]
impl<T: AsyncProtocolStream> AsyncProtocolStreamSplit for EncryptedStream<T> {
    type OwnedSplitReadHalf = EncryptedReadStream<T::OwnedSplitReadHalf>;
    type OwnedSplitWriteHalf = EncryptedWriteStream<T::OwnedSplitWriteHalf>;

    fn protocol_into_split(self) -> (Self::OwnedSplitReadHalf, Self::OwnedSplitWriteHalf) {
        (self.read_half, self.write_half)
    }
}
