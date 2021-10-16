#[macro_export]
macro_rules! callback {
    ($cb:ident) => {
        |ctx, event| Box::pin($cb(ctx, event))
    };
    ($ctx:ident, $event:ident,$cb:expr) => {
        move |$ctx, $event| Box::pin($cb)
    };
}
