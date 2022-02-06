use crate::context::Context;
use crate::error;
use crate::error::Error;
use crate::event::{Event, EventType};
use crate::payload::IntoPayload;
use std::mem;

pub(crate) struct EventMetadata<P: IntoPayload> {
    pub(crate) event: Option<Event>,
    pub(crate) ctx: Option<Context>,
    pub(crate) event_namespace: Option<Option<String>>,
    pub(crate) event_name: Option<String>,
    pub(crate) res_id: Option<Option<u64>>,
    pub(crate) event_type: Option<EventType>,
    pub(crate) payload: Option<P>,
}

impl<P: IntoPayload> EventMetadata<P> {
    pub fn get_event(&mut self) -> error::Result<&Event> {
        if self.event.is_none() {
            self.build_event()?;
        }
        Ok(self.event.as_ref().unwrap())
    }

    pub fn take_event(mut self) -> error::Result<Option<Event>> {
        if self.event.is_none() {
            self.build_event()?;
        }
        Ok(mem::take(&mut self.event))
    }

    fn build_event(&mut self) -> error::Result<()> {
        let ctx = self.ctx.take().ok_or(Error::InvalidState)?;
        let event = self.event_name.take().ok_or(Error::InvalidState)?;
        let namespace = self.event_namespace.take().ok_or(Error::InvalidState)?;
        let payload = self.payload.take().ok_or(Error::InvalidState)?;
        let res_id = self.res_id.take().ok_or(Error::InvalidState)?;
        let event_type = self.event_type.take().ok_or(Error::InvalidState)?;
        let payload_bytes = payload.into_payload(&ctx)?;

        let event = Event::new(
            namespace,
            event.to_string(),
            payload_bytes,
            res_id,
            event_type,
        );

        self.event = Some(event);

        Ok(())
    }
}
