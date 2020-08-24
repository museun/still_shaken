use crate::{
    error::*,
    responder::{Responder, Response},
    Config,
};

use async_channel::Receiver;
use std::{future::Future, sync::Arc};
use twitchchat::{messages::Privmsg, runner::Identity};

mod runner;
pub use runner::Runner;

mod tasks;
use tasks::Tasks;

mod modules;

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
        smol::Task::spawn(fut)
    }
}

pub struct Context {
    pub identity: Arc<Identity>,
    pub stream: Receiver<Arc<Privmsg<'static>>>,
    pub responder: Responder,
}

pub trait PrivmsgExt {
    fn is_mentioned(&self, identity: &Identity) -> bool;
    fn user_name(&self) -> &str;
}

impl<'a> PrivmsgExt for Privmsg<'a> {
    fn is_mentioned(&self, identity: &Identity) -> bool {
        let username = identity.username();
        match self.data().splitn(2, char::is_whitespace).next() {
            Some(s) if s.starts_with('@') && s.ends_with(username) => true,
            Some(s) if s.starts_with(username) && s.ends_with(':') => true,
            _ => false,
        }
    }
    fn user_name(&self) -> &str {
        self.display_name().unwrap_or_else(|| self.name())
    }
}

pub struct Cmd<'a> {
    pub head: &'a str,
    pub arg: Option<&'a str>,
    pub body: Option<&'a str>,
}

impl<'a> Cmd<'a> {
    pub fn parse(msg: &'a Privmsg<'_>) -> Option<Self> {
        const LEADER: &str = "!";

        if !msg.data().starts_with(LEADER) || msg.data().len() == LEADER.len() {
            return None;
        }

        let mut iter = msg
            .data()
            .get(LEADER.len()..)?
            .splitn(3, char::is_whitespace);

        let head = iter.next()?;
        let arg = iter.next().and_then(|c| match c {
            LEADER => None,
            c if c.starts_with(LEADER) => c.get(LEADER.len()..),
            c => Some(c),
        });
        let body = iter.next();

        Some(Self { head, arg, body })
    }
}
