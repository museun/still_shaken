use super::Context;
use super::Executor;

use async_executor::Task;
use std::future::Future;

pub trait Handler {
    fn spawn(self, context: Context, executor: Executor) -> Task<()>;
}

impl<F, R> Handler for F
where
    F: Fn(Context) -> R + Send + Sync,
    R: Future<Output = ()> + Send + Sync + 'static,
    R::Output: Send + Sync + 'static,
{
    fn spawn(self, context: Context, executor: Executor) -> Task<()> {
        let fut = (self)(context);
        executor.spawn(fut)
    }
}
