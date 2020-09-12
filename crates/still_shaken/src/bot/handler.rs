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

pub struct ContextState {
    pub responder: Responder,
    pub state: Arc<Mutex<State>>,
    pub identity: Arc<Identity>,
    pub executor: Executor,
}

#[derive(Clone)]
pub struct Context<A = Privmsg<'static>> {
    pub args: Arc<A>,
    pub state: Arc<ContextState>,
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
    pub fn new(args: A, state: Arc<ContextState>) -> Self {
        Self {
            args: Arc::new(args),
            state,
        }
    }

    pub fn responder(&self) -> &Responder {
        &self.state.responder
    }

    // TODO return a guard for this
    pub fn state(&self) -> &Mutex<State> {
        &self.state.state
    }

    pub fn identity(&self) -> &Identity {
        &self.state.identity
    }

    pub fn executor(&self) -> &Executor {
        &self.state.executor
    }
}

impl Context<Privmsg<'static>> {
    // pub fn parts(&self) {}
    // pub fn command(&self) {}
}

impl Context<CommandArgs> {
    pub fn channel(&self) -> &str {
        self.args.msg.channel()
    }
}
