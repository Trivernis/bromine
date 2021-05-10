use crate::ipc::stream_emitter::StreamEmitter;

pub struct Context {
    pub emitter: StreamEmitter,
}

impl Context {
    pub fn new(emitter: StreamEmitter) -> Self {
        Self { emitter }
    }
}
