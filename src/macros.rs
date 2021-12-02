#[macro_export]
macro_rules! callback {
    ($cb:ident) => {
        |ctx, event| Box::pin($cb(ctx, event))
    };
    ($cb:path) => {
        |ctx, event| Box::pin($cb(ctx, event))
    };
    ($ctx:ident, $event:ident,$cb:expr) => {
        move |$ctx, $event| Box::pin($cb)
    };
}

#[macro_export]
macro_rules! namespace {
    ($nsp:path) => {
        Namespace::from_provider::<$nsp>()
    };
}

#[macro_export]
macro_rules! events{
    ($handler:expr, $($name:expr => $cb:ident), *) => {
        $(
            $handler.on($name, callback!($cb));
        )*
    };
    ($handler:expr, $($name:expr => $cb:path), *) => {
        $(
            $handler.on($name, callback!($cb));
        )*
    }
}
