use super::{Handler, Responder};

use async_channel::Sender;
use twitchchat::{messages::Privmsg, runner::Identity};

use std::{collections::BTreeSet, future::Future, sync::Arc, time::Instant};

#[derive(Clone)]
pub struct Executor {
    inner: Arc<async_executor::Executor>,
}

impl Executor {
    pub fn new(threads: usize) -> (Self, std::thread::JoinHandle<()>, async_channel::Sender<()>) {
        let ex = Arc::new(async_executor::Executor::new());
        let (stop_tx, stop_rx) = async_channel::bounded(1);

        let handle = std::thread::spawn({
            let ex = Arc::clone(&ex);
            move || {
                easy_parallel::Parallel::new()
                    .each(0..threads, {
                        let ex = Arc::clone(&ex);
                        move |_| futures_lite::future::block_on(ex.run(stop_rx.recv()))
                    })
                    .finish(|| {
                        futures_lite::future::block_on(async move {
                            log::info!("stopping executors");
                        })
                    });
            }
        });

        (Self { inner: ex }, handle, stop_tx)
    }
}

impl Executor {
    pub fn spawn<F, T>(&self, fut: F) -> async_executor::Task<T>
    where
        F: Future<Output = T> + Send + Sync + 'static,
        T: Send + Sync + 'static,
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
        C: Handler + Send + Sync,
    {
        self.spawn(cmd);
        self
    }

    pub fn spawn<C>(&mut self, cmd: C)
    where
        C: Handler + Send + Sync,
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
