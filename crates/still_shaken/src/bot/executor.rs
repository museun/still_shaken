use std::{future::Future, sync::Arc};

#[derive(Clone)]
pub struct Executor {
    inner: Arc<async_executor::Executor>,
}

impl Executor {
    pub fn new(threads: usize) -> Self {
        let ex = Arc::new(async_executor::Executor::new());

        for i in 1..=threads {
            std::thread::Builder::new()
                .name(format!("still_shaken-{}", i))
                .spawn({
                    let ex = Arc::clone(&ex);
                    log::debug!("spawning executor thread");
                    move || loop {
                        let _ = std::panic::catch_unwind(|| {
                            async_io::block_on(ex.run(futures_lite::future::pending::<()>()))
                        });

                        log::debug!("end of executor thread ({})", i);
                    }
                })
                .expect("named thread support");
        }

        Self { inner: ex }
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
