use super::{Handler, Responder};

use async_channel::Sender;
use twitchchat::{messages::Privmsg, runner::Identity};

use std::{collections::BTreeSet, future::Future, sync::Arc, time::Instant};

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

pub struct Tasks {
    tasks: Vec<(Instant, Task)>,
    identity: Arc<Identity>,
    responder: Responder,
    executor: Executor,
}

impl Tasks {
    pub fn new<I>(responder: Responder, identity: I, executor: Executor) -> Self
    where
        I: Into<Arc<Identity>>,
    {
        Self {
            tasks: Vec::new(),
            responder,
            identity: identity.into(),
            executor,
        }
    }

    pub fn with<C>(mut self, cmd: C) -> Self
    where
        C: Handler + Send,
    {
        self.spawn(cmd);
        self
    }

    pub fn spawn<C>(&mut self, cmd: C)
    where
        C: Handler + Send,
    {
        let (tx, stream) = async_channel::bounded(32);

        let context = super::Context {
            identity: self.identity.clone(),
            responder: self.responder.clone(),
            stream,
        };

        let task = Task {
            inner: cmd.spawn(context, self.executor.clone()),
            sink: tx,
        };

        let now = Instant::now();
        self.tasks.push((now, task));
    }

    pub fn send_all<M>(&mut self, msg: M)
    where
        M: Into<Arc<Privmsg<'static>>>,
    {
        let msg = msg.into();

        let mut bad = BTreeSet::<Instant>::new();
        for (id, task) in &self.tasks {
            if let Err(async_channel::TrySendError::Closed(..)) = task.sink.try_send(msg.clone()) {
                bad.insert(*id);
            }
        }

        // check before we even try it
        if !bad.is_empty() {
            // inverted so we remove the bad ones
            self.tasks.retain(|(id, _)| !bad.remove(id))
        }
    }

    pub async fn cancel_remaining(self) {
        for (_, task) in self.tasks {
            let _ = task.inner.cancel().await;
        }
    }
}

struct Task {
    inner: async_executor::Task<()>,
    sink: Sender<Arc<Privmsg<'static>>>,
}
