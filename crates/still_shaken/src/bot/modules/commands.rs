use super::{Cmd, Context, DontCare, Handler, Responder};
use crate::{
    config,
    format::FormatTime,
    template::{Environment, SimpleTemplate, Template},
    util::PrivmsgExt as _,
};

use futures_lite::StreamExt as _;
use twitchchat::messages::Privmsg;

use std::collections::HashMap;

pub struct Commands {
    config: config::Commands, // TODO make this reloadable at runtime
    channels: HashMap<String, Channel>,
    start: std::time::Instant,
}

impl Handler for Commands {
    fn spawn(mut self, mut context: Context) -> smol::Task<()> {
        smol::Task::spawn(async move {
            while let Some(msg) = context.stream.next().await {
                let _ = self
                    .handle(&*msg, &mut context.responder)
                    .is_real_error()
                    .map(|err| log::error!("commands: {}", err));
            }
        })
    }
}

impl Commands {
    pub fn new(config: &config::Commands) -> Self {
        let file = &config.commands_file;
        let map = data::load_saved(file).unwrap_or_default();

        // TODO load default formatters
        let channels = map
            .channels
            .into_iter()
            .map(|(k, ch)| (k, Channel::from_saved(ch)))
            .collect();

        Self {
            config: config.clone(),
            channels,
            start: std::time::Instant::now(),
        }
    }

    fn handle(&mut self, msg: &Privmsg<'_>, responder: &mut Responder) -> anyhow::Result<()> {
        let Cmd { head, arg, body } = match Cmd::parse(msg) {
            Some(cmd) => cmd,
            None => really_dont_care!(),
        };

        let elevated = msg.is_broadcaster() || msg.is_moderator();

        match (head, arg) {
            ("add", Some(arg)) if elevated => {
                if let Some(ch) = self.channels.get(msg.channel()) {
                    if ch.commands.contains_key(arg) {
                        return responder.reply(msg, format!("'{}' already exists", arg));
                    }
                }
                self.update_template(msg, responder, arg, body)?;
                return responder.nothing();
            }

            ("edit", Some(arg)) if elevated => {
                let ch = self.channels.get(msg.channel());
                if ch.is_none() || !ch.unwrap().commands.contains_key(arg) {
                    return responder.reply(msg, format!("'{}' does not exist", arg));
                }
                self.update_template(msg, responder, arg, body)?;
                return responder.nothing();
            }

            ("remove", Some(arg)) if elevated => {
                let out = match self.channels.get_mut(msg.channel()) {
                    Some(ch) => {
                        if !ch.remove_command(arg) {
                            format!("'{}' does not exist", arg)
                        } else {
                            format!("removed '{}'", arg)
                        }
                    }
                    None => format!("'{}' does not exist", arg),
                };

                if let Err(err) = self.sync_commands() {
                    log::error!("cannot sync commands: {}", err);
                }

                return responder.reply(msg, out);
            }
            _ => {}
        }

        if let Some(template) = self
            .channels
            .get(msg.channel())
            .and_then(|ch| ch.commands.get(head))
        {
            let env = Environment::default()
                .insert("name", msg.user_name().to_string())
                .insert("channel", msg.channel().to_string())
                .insert("uptime", self.start.elapsed().relative_time());
            let resp = template.apply(&env);
            return responder.say(msg, resp);
        }

        responder.nothing()
    }

    fn sync_commands(&self) -> anyhow::Result<()> {
        let channels = self.channels.iter().map(|(k, v)| {
            let channel = data::Channel {
                commands: v
                    .commands
                    .iter()
                    .map(|(k, v)| (k.clone(), v.body().to_string()))
                    .collect(),
            };
            (k.to_string(), channel)
        });

        let saved = data::Saved {
            channels: channels.collect(),
        };

        let s = toml::to_string_pretty(&saved)?;
        std::fs::write(&self.config.commands_file, &s)?;
        Ok(())
    }

    fn update_template(
        &mut self,
        msg: &Privmsg<'_>,
        responder: &mut Responder,
        cmd: &str,
        body: Option<&str>,
    ) -> anyhow::Result<()> {
        let body = match body {
            Some(body) => body,
            None => return responder.reply(msg, "try again. you provided an empty command body"),
        };

        if body.starts_with('.') | body.starts_with('/') {
            return responder.reply(msg, "lol");
        }

        log::info!(
            "updating template: {} -> {}",
            cmd.escape_debug(),
            body.escape_debug()
        );

        responder.reply(msg, format!("updated '{}' -> '{}'", cmd, body))?;

        self.channels
            .entry(msg.channel().to_string())
            .or_default()
            .add_template(cmd, SimpleTemplate::new(cmd, body));

        self.sync_commands()?;
        Ok(())
    }
}

#[derive(Default)]
struct Channel {
    commands: HashMap<String, Box<dyn Template>>,
}

impl Channel {
    fn add_template<N, T>(&mut self, name: N, template: T)
    where
        N: Into<String>,
        T: Template + 'static,
    {
        self.commands.insert(name.into(), Box::new(template));
    }

    fn remove_command(&mut self, cmd: &str) -> bool {
        self.commands.remove(cmd).is_some()
    }

    fn from_saved(saved: data::Channel) -> Self {
        let commands = saved.commands.into_iter().map(|(k, v)| {
            let t = SimpleTemplate::new(&k, v);
            let t: Box<dyn Template> = Box::new(t);
            (k, t)
        });
        Self {
            commands: commands.collect(),
        }
    }
}

mod data {
    use std::collections::HashMap;

    #[derive(Default, serde::Deserialize, serde::Serialize)]
    pub struct Channel {
        pub commands: HashMap<String, String>,
    }

    #[derive(Default, serde::Deserialize, serde::Serialize)]
    pub struct Saved {
        #[serde(flatten)]
        pub channels: HashMap<String, Channel>,
    }

    pub fn load_saved(file: &str) -> anyhow::Result<Saved> {
        let s = std::fs::read_to_string(file)?;
        let t = toml::from_str(&s)?;
        Ok(t)
    }
}
