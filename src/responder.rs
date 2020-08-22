use std::io::{Result, Write};
use twitchchat::{commands, messages::Privmsg};

use async_channel::Sender;

#[derive(Clone)]
pub struct Responder {
    sender: Sender<Response>,
}

impl Responder {
    pub const fn new(sender: Sender<Response>) -> Self {
        Self { sender }
    }

    pub fn say(&self, msg: &Privmsg<'_>, resp: impl Into<String>) {
        let _ = self.sender.try_send(Response::Say(Say {
            channel: msg.channel().to_string().into_boxed_str(),
            data: resp.into().into_boxed_str(),
        }));
    }

    pub fn reply(&self, msg: &Privmsg<'_>, resp: impl Into<String>) {
        let _ = self.sender.try_send(Response::Reply(Reply {
            channel: msg.channel().to_string().into_boxed_str(),
            msg_id: msg.tags().get("id").unwrap().to_string().into_boxed_str(),
            data: resp.into().into_boxed_str(),
        }));
    }

    pub fn nothing(&self) {}
}

#[derive(Debug)]
pub enum Response {
    Reply(Reply),
    Say(Say),
}

#[derive(Debug)]
struct Reply {
    channel: Box<str>,
    msg_id: Box<str>,
    data: Box<str>,
}

#[derive(Debug)]
struct Say {
    channel: Box<str>,
    data: Box<str>,
}

impl twitchchat::Encodable for Reply {
    fn encode<W>(&self, buf: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        commands::reply(&self.channel, &self.msg_id, &self.data).encode(buf)
    }
}

impl twitchchat::Encodable for Say {
    fn encode<W>(&self, buf: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        commands::privmsg(&self.channel, &self.data).encode(buf)
    }
}

impl twitchchat::Encodable for Response {
    fn encode<W>(&self, buf: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        match self {
            Response::Reply(reply) => reply.encode(buf),
            Response::Say(say) => say.encode(buf),
        }
    }
}
