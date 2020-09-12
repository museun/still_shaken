use std::io::{Result, Write};
use twitchchat::{commands, messages::Privmsg, Encodable};

use async_channel::Sender;

#[derive(Clone)]
pub struct Responder {
    sender: Sender<Response>,
}

impl Responder {
    pub const fn new(sender: Sender<Response>) -> Self {
        Self { sender }
    }

    pub fn say<R>(&self, msg: &Privmsg<'_>, resp: R) -> anyhow::Result<()>
    where
        R: Into<String>,
    {
        let data: Box<str> = resp.into().trim().into();
        let say = Say {
            channel: msg.channel().into(),
            data,
        };
        log::debug!("say: {:?}", say);
        self.sender.try_send(Response::Say(say))?;
        Ok(())
    }

    pub fn reply<R>(&self, msg: &Privmsg<'_>, resp: R) -> anyhow::Result<()>
    where
        R: Into<String>,
    {
        let data: Box<str> = resp.into().trim().into();
        let reply = Reply {
            channel: msg.channel().into(),
            msg_id: msg.tags().get("id").unwrap().into(),
            data,
        };
        log::debug!("reply: {:?}", reply);
        self.sender.try_send(Response::Reply(reply))?;
        Ok(())
    }

    pub fn nothing(&self) -> anyhow::Result<()> {
        crate::error::dont_care()
    }
}

#[derive(Debug)]
pub enum Response {
    Reply(Reply),
    Say(Say),
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = Vec::new();
        self.encode(&mut buf).unwrap();
        f.write_str(std::str::from_utf8(&buf).unwrap())
    }
}

#[derive(Debug)]
pub struct Reply {
    pub channel: Box<str>,
    pub msg_id: Box<str>,
    pub data: Box<str>,
}

#[derive(Debug)]
pub struct Say {
    pub channel: Box<str>,
    pub data: Box<str>,
}

impl twitchchat::Encodable for Reply {
    fn encode<W>(&self, buf: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        commands::reply(&self.channel, &self.msg_id, &self.data).encode(buf)?;
        buf.flush()
    }
}

impl twitchchat::Encodable for Say {
    fn encode<W>(&self, buf: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        commands::privmsg(&self.channel, &self.data).encode(buf)?;
        buf.flush()
    }
}

impl twitchchat::Encodable for Response {
    fn encode<W>(&self, buf: &mut W) -> Result<()>
    where
        W: Write + ?Sized,
    {
        match self {
            Self::Reply(reply) => reply.encode(buf),
            Self::Say(say) => say.encode(buf),
        }?;
        buf.flush()
    }
}
