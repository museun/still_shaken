use super::{Handler, Responder};

use async_channel::Sender;
use twitchchat::{messages::Privmsg, runner::Identity};

use std::{collections::BTreeSet, sync::Arc, time::Instant};

pub struct Tasks {
    tasks: Vec<(Instant, Task)>,
    identity: Arc<Identity>,
    responder: Responder,
}

impl Tasks {
    pub fn new<I>(responder: Responder, identity: I) -> Self
    where
        I: Into<Arc<Identity>>,
    {
        Self {
            tasks: Vec::new(),
            responder,
            identity: identity.into(),
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
            inner: cmd.spawn(context),
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
    inner: smol::Task<()>,
    sink: Sender<Arc<Privmsg<'static>>>,
}
