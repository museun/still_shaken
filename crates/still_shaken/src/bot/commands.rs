use super::{Cmd, Context, Handler, Responder};
use crate::{
    config,
    template::{Environment, SimpleTemplate, Template},
};

use futures_lite::StreamExt as _;
use twitchchat::messages::Privmsg;

use std::collections::HashMap;

pub struct Commands {
    env: Environment,
    config: config::Commands, // TODO make this reloadable at runtime
    channels: HashMap<String, Channel>,
}

impl Handler for Commands {
    fn spawn(mut self, mut context: Context) -> smol::Task<()> {
        smol::Task::spawn(async move {
            while let Some(msg) = context.stream.next().await {
                let _ = self.handle(&*msg, &mut context.responder);
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
            env: Environment::default(),
            config: config.clone(),
            channels,
        }
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
    ) {
        let body = match body {
            Some(body) => body,
            None => {
                responder.reply(msg, "try again. you provided an empty command body");
                return;
            }
        };

        if body.starts_with('.') | body.starts_with('/') {
            responder.reply(msg, "lol");
            return;
        }

        log::info!(
            "updating template: {} -> {}",
            cmd.escape_debug(),
            body.escape_debug()
        );

        responder.reply(msg, format!("updated '{}' -> '{}'", cmd, body));

        let template = SimpleTemplate::new(cmd, body);

        self.channels
            .entry(msg.channel().to_string())
            .or_default()
            .add_template(cmd, SimpleTemplate::new(cmd, body));

        if let Err(err) = self.sync_commands() {
            log::error!("cannot save commands: {}", err)
        }
    }

    fn handle(&mut self, msg: &Privmsg<'_>, responder: &mut Responder) {
        let Cmd { head, arg, body } = match Cmd::parse(msg) {
            Some(p) => p,
            None => return,
        };

        let elevated = msg.is_broadcaster() || msg.is_moderator();

        match (head, arg) {
            ("add", Some(arg)) if elevated => {
                if let Some(ch) = self.channels.get(msg.channel()) {
                    if ch.commands.contains_key(arg) {
                        responder.reply(msg, format!("'{}' already exists", arg));
                        return;
                    }
                }
                self.update_template(msg, responder, arg, body);
                return;
            }

            ("edit", Some(arg)) if elevated => {
                let ch = self.channels.get(msg.channel());
                if ch.is_none() || !ch.unwrap().commands.contains_key(arg) {
                    responder.reply(msg, format!("'{}' does not exist", arg));
                    return;
                }
                self.update_template(msg, responder, arg, body);
                return;
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

                responder.reply(msg, out);
                return;
            }
            _ => {}
        }

        if let Some(resp) = self
            .channels
            .get(msg.channel())
            .and_then(|ch| ch.commands.get(head))
            .map(|s| s.apply(&self.env))
        {
            responder.say(msg, resp);
        }
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
