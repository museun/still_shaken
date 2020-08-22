use crate::{
    responder::{Responder, Response},
    Config,
};

use async_channel::Receiver;
use std::sync::Arc;
use twitchchat::{messages::Privmsg, runner::Identity};

mod runner;
pub use runner::Runner;

mod shaken;
use shaken::Shaken;

mod commands;
use commands::Commands;

mod tasks;
use tasks::Tasks;

pub type Writer = twitchchat::writer::AsyncWriter<twitchchat::writer::MpscWriter>;

pub trait Handler {
    fn spawn(self, context: Context) -> smol::Task<()>;
}

pub struct Context {
    pub identity: Arc<Identity>,
    pub stream: Receiver<Arc<Privmsg<'static>>>,
    pub responder: Responder,
}

pub trait PrivmsgExt {
    fn is_mentioned(&self, identity: &Identity) -> bool;
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
