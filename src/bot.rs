use crate::{
    responder::{Responder, Response},
    Config,
};

use async_channel::Receiver;
use twitchchat::messages::Privmsg;

use std::sync::Arc;

mod runner;
pub use runner::Runner;

mod shaken;
use shaken::Shaken;

mod tasks;
use tasks::Tasks;

// pub type AnyhowFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + Sync + 'a>>;

pub type Writer = twitchchat::writer::AsyncWriter<twitchchat::writer::MpscWriter>;

pub type Recv = Receiver<Arc<Privmsg<'static>>>;

pub trait Handler {
    fn sink(self, recv: Recv, responder: Responder) -> smol::Task<()>;
}
