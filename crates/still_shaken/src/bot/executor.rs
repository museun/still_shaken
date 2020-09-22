use futures_lite::future;
use std::{future::Future, sync::Arc};

#[derive(Clone)]
pub struct Executor {
    inner: Arc<async_executor::Executor<'static>>,
}

impl Executor {
    /// TODO allow this to shutdown via a channel
    pub fn new(threads: usize) -> Self {
        let inner = Arc::new(async_executor::Executor::new());

        for i in 1..=threads {
            std::thread::Builder::new()
                .name(format!("still_shaken-{}", i))
                .spawn({
                    let ex = Arc::clone(&inner);
                    log::debug!("spawning executor thread");
                    move || loop {
                        let _ = std::panic::catch_unwind(|| {
                            let fut = ex.run(future::pending::<()>());
                            future::block_on(fut)
                        });
                        log::debug!("end of executor thread ({})", i);
                    }
                })
                .expect("named thread support");
        }

        Self { inner }
    }
}

impl Executor {
    pub fn spawn<F, T>(&self, fut: F) -> async_executor::Task<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.inner.spawn(fut)
    }
}
