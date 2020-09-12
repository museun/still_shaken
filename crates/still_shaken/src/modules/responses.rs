use crate::*;

use async_mutex::Mutex;

use error::DontCare;
use responder::Responder;
use shaken_template::{Environment, SimpleTemplate, Template};
use twitchchat::messages::Privmsg;

use std::{collections::HashMap, sync::Arc};

pub struct Responses {
    config: config::Commands,
    channels: Mutex<HashMap<String, Channel>>,
}

impl super::Initialize for Responses {
    fn initialize(
        config: &Config,
        commands: &mut Commands,
        passives: &mut Passives,
        _executor: &Executor,
    ) -> anyhow::Result<()> {
        let s = Arc::new(Self::new(&config.modules.commands));

        commands.elevated(s.clone(), "!set <command> <body...>", Self::set_command)?;
        commands.elevated(s.clone(), "!add <command> <body...>", Self::add_command)?;
        commands.elevated(s.clone(), "!edit <command> <body...>", Self::edit_command)?;
        commands.elevated(s.clone(), "!remove <command>", Self::remove_command)?;
        passives.with(s, Self::handle);

        Ok(())
    }
}

impl Responses {
    async fn handle(self: Arc<Self>, ctx: Context<Privmsg<'static>>) -> anyhow::Result<()> {
        fn get_cmd(data: &str) -> Option<&str> {
            if !data.starts_with(Command::LEADER) {
                return None;
            }
            data.trim_start_matches(Command::LEADER)
                .split_whitespace()
                .next()
        }

        let msg = ctx.msg();
        let head = get_cmd(msg.data()).dont_care()?;

        let channels = self.channels.lock().await;
        let template = channels
            .get(msg.channel())
            .dont_care()?
            .commands
            .get(head)
            .dont_care()?;

        let (name, channel) = (msg.user_name(), msg.channel());
        let env = Environment::default()
            .insert("name", &name)
            .insert("channel", &channel);

        ctx.say(template.apply(&env)?)
    }

    async fn set_command(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let cmd = &ctx.args["command"];
        let body = ctx.args.get_non_empty("body");

        let msg = &ctx.args.msg;
        let responder = &ctx.responder();
        self.update_template(msg, responder, cmd, body).await
    }

    async fn add_command(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let cmd = &ctx.args["command"];
        let body = ctx.args.get_non_empty("body");

        if let Some(ch) = self.channels.lock().await.get(ctx.channel()) {
            if ch.commands.contains_key(&*cmd) {
                return ctx.reply(format!("'{}' already exists", cmd));
            }
        }

        let msg = &ctx.args.msg;
        let responder = &ctx.responder();
        self.update_template(msg, responder, cmd, body).await
    }

    async fn edit_command(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let cmd = &ctx.args["command"];
        let body = ctx.args.get_non_empty("body");

        {
            let channels = self.channels.lock().await;
            let ch = channels.get(ctx.channel());
            if ch.is_none() || !ch.unwrap().commands.contains_key(&*cmd) {
                return ctx.reply(format!("'{}' does not exist", cmd));
            }
        }

        let msg = &ctx.args.msg;
        let responder = &ctx.responder();
        self.update_template(msg, responder, cmd, body).await
    }

    async fn remove_command(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let cmd = &ctx.args["command"];

        let out = match self.channels.lock().await.get_mut(ctx.channel()) {
            Some(ch) => {
                if ch.remove_command(&*cmd) {
                    format!("'{}' does not exist", cmd)
                } else {
                    format!("removed '{}'", cmd)
                }
            }
            None => format!("'{}' does not exist", cmd),
        };

        ctx.reply(out)
    }
}

impl Responses {
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
            channels: Mutex::new(channels),
        }
    }

    async fn sync_commands(&self) -> anyhow::Result<()> {
        let channels = self.channels.lock().await;
        let channels = channels.iter().map(|(k, v)| {
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

    async fn update_template(
        &self,
        msg: &Privmsg<'_>,
        responder: &Responder,
        cmd: &str,
        body: Option<&str>,
    ) -> anyhow::Result<()> {
        let body = match body {
            Some(body) => body,
            None => return responder.reply(msg, "try again. you provided an empty command body"),
        };

        if body.starts_with('.') || body.starts_with('/') {
            return responder.reply(msg, "lol");
        }

        log::info!(
            "updating template: {} -> {}",
            cmd.escape_debug(),
            body.escape_debug()
        );

        responder.reply(msg, format!("updated '{}' -> '{}'", cmd, body))?;

        self.channels
            .lock()
            .await
            .entry(msg.channel().to_string())
            .or_default()
            .add_template(cmd, SimpleTemplate::new(cmd, body));

        self.sync_commands().await?;
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

pub fn get_commands(config: &Config, channel: &str) -> anyhow::Result<Vec<(String, String)>> {
    let saved = data::load_saved(&config.modules.commands.commands_file)?;

    Ok(saved
        .channels
        .get(channel)
        .map(|channel| channel.commands.clone().into_iter().collect())
        .unwrap_or_default())
}
