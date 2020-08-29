use super::Context;
use std::future::Future;

pub trait Handler {
    fn spawn(self, context: Context) -> smol::Task<()>;
}

impl<F, R> Handler for F
where
    F: Fn(Context) -> R + Send + Sync,
    R: Future<Output = ()> + Send + Sync + 'static,
    R::Output: Send + Sync + 'static,
{
    fn spawn(self, context: Context) -> smol::Task<()> {
        let fut = (self)(context);
        smol::spawn(fut)
    }
}
