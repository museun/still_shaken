use super::Responder;

use async_channel::Receiver;
use twitchchat::{messages::Privmsg, runner::Identity};

use std::sync::Arc;

pub struct Context {
    pub identity: Arc<Identity>,
    pub stream: Receiver<Arc<Privmsg<'static>>>,
    pub responder: Responder,
}
