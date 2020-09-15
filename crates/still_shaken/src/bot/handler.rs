use super::{state::State, CommandArgs};
use crate::{responder::Responder, Executor};

use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

use async_mutex::Mutex;
use twitchchat::{messages::Privmsg, runner::Identity};

pub trait Callable<Args>
where
    Self: Send + 'static,
    Args: Send + 'static,
{
    type Fut: Future<Output = anyhow::Result<()>> + Send + 'static;
    fn call(&self, state: Context<Args>) -> Self::Fut;
}

pub type AnyhowFut<'t> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 't>>;

impl<F, Fut, Args> Callable<Args> for F
where
    F: Fn(Context<Args>) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    Args: Send + 'static,
{
    type Fut = AnyhowFut<'static>;

    fn call(&self, state: Context<Args>) -> Self::Fut {
        Box::pin((self)(state))
    }
}

pub struct Context<A = Privmsg<'static>> {
    pub args: Arc<A>,
    pub responder: Responder,
    pub state: Arc<Mutex<State>>,
    pub identity: Arc<Identity>,
    pub executor: Executor,
}

impl<A> Clone for Context<A> {
    fn clone(&self) -> Context<A> {
        Self {
            args: self.args.clone(),
            responder: self.responder.clone(),
            state: self.state.clone(),
            identity: self.identity.clone(),
            executor: self.executor.clone(),
        }
    }
}

impl<A> Debug for Context<A>
where
    A: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").field("args", &self.args).finish()
    }
}

impl<A> Context<A> {
    pub fn mapped<B>(&self, args: B) -> Context<B> {
        let Context {
            responder,
            state,
            identity,
            executor,
            ..
        } = self.clone();

        Context {
            args: Arc::new(args),
            responder,
            state,
            identity,
            executor,
        }
    }

    pub fn new(
        args: A,
        responder: Responder,
        state: Arc<Mutex<State>>,
        identity: Arc<Identity>,
        executor: Executor,
    ) -> Self {
        Self {
            args: Arc::new(args),
            responder,
            state,
            identity,
            executor,
        }
    }

    pub fn responder(&self) -> &Responder {
        &self.responder
    }

    // TODO return a guard for this
    pub fn state(&self) -> &Mutex<State> {
        &self.state
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn executor(&self) -> &Executor {
        &self.executor
    }
}

pub trait Respond {
    fn msg(&self) -> &Privmsg<'static>;
    fn responder(&self) -> &Responder;

    fn say<R>(&self, resp: R) -> anyhow::Result<()>
    where
        R: Into<String>,
    {
        self.responder().say(self.msg(), resp)
    }

    fn reply<R>(&self, resp: R) -> anyhow::Result<()>
    where
        R: Into<String>,
    {
        self.responder().reply(self.msg(), resp)
    }
}

impl Respond for Context<Privmsg<'static>> {
    fn msg(&self) -> &Privmsg<'static> {
        &*self.args
    }

    fn responder(&self) -> &Responder {
        &self.responder
    }
}

impl Respond for Context<CommandArgs> {
    fn msg(&self) -> &Privmsg<'static> {
        &*self.args.msg
    }

    fn responder(&self) -> &Responder {
        &self.responder
    }
}

impl Context<CommandArgs> {
    pub fn channel(&self) -> &str {
        self.msg().channel()
    }
}
