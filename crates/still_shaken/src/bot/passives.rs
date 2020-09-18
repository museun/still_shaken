use crate::{ActiveCallable, AnyhowFut, Callable, Context, Executor};

use std::{future::Future, sync::Arc};
use twitchchat::messages::Privmsg;

pub struct Passives {
    executor: Executor,
    callables: Vec<Box<ActiveCallable>>,
}

impl Passives {
    pub fn new(executor: Executor) -> Self {
        Self {
            executor,
            callables: Vec::new(),
        }
    }

    pub fn with<T, Fut, F>(&mut self, this: Arc<T>, func: F)
    where
        T: Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
        Fut: Send + Sync + 'static,
        F: Fn(Arc<T>, Context<Privmsg<'static>>) -> Fut,
        F: Send + Sync + 'static,
    {
        self.add(Box::new(move |ctx| func(this.clone(), ctx)))
    }

    pub fn add<H, F>(&mut self, callable: H)
    where
        H: Callable<Privmsg<'static>, Fut = F> + 'static,
        F: Future<Output = anyhow::Result<()>>,
        F: Send + Sync + 'static,
    {
        self.callables.push(Box::new(move |ctx| callable.call(ctx)));
    }
}

impl Callable<Privmsg<'static>> for Passives {
    type Fut = AnyhowFut<'static>;

    fn call(&self, state: Context<Privmsg<'static>>) -> Self::Fut {
        self.callables.iter().for_each(|callable| {
            let fut = callable.call(state.clone());
            let task = self.executor.spawn(fut);
            // TODO don't just leak these
            task.detach()
        });
        Box::pin(async move { Ok(()) })
    }
}
