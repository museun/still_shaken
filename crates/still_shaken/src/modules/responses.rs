use crate::*;

use super::{Components, Initialize};
use persist::{Persist, Toml};
use responder::Responder;

use shaken_commands::Command;
use shaken_template::{Environment, SimpleTemplate, Template};

use async_mutex::Mutex;
use std::{collections::HashMap, sync::Arc};
use twitchchat::messages::Privmsg;

/// This is used to get the command from other modules
pub fn get_commands(config: &Config, channel: &str) -> anyhow::Result<Vec<(String, String)>> {
    data::load_saved(&config.modules.commands.commands_file).map(|saved| {
        saved
            .channels
            .get(channel)
            .map(|channel| channel.commands.clone().into_iter().collect())
            .unwrap_or_default()
    })
}

pub struct Responses {
    config: config::Commands,
    channels: Mutex<HashMap<String, Channel>>,
}

impl Initialize for Responses {
    fn initialize(
        Components {
            config,
            commands,
            passives,
            ..
        }: &mut Components<'_>,
    ) -> anyhow::Result<()> {
        let s = Arc::new(Self::new(&config.modules.commands));

        commands.elevated(s.clone(), "!add <command> <body...>", Self::add_command)?;
        commands.elevated(s.clone(), "!remove <command>", Self::remove_command)?;
        commands.elevated(s.clone(), "!set <command> <body...>", Self::set_command)?;
        passives.with(s, Self::handle);

        Ok(())
    }
}

impl Responses {
    async fn handle(self: Arc<Self>, ctx: Context<Privmsg<'static>>) -> anyhow::Result<()> {
        fn get_cmd(data: &str) -> Option<&str> {
            if !data.starts_with(shaken_commands::Command::LEADER) {
                return None;
            }
            data.trim_start_matches(shaken_commands::Command::LEADER)
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
        let cmd = Self::get_command(&ctx);
        let body = ctx.args.get_non_empty("body");

        let action = if self
            .channels
            .lock()
            .await
            .get(ctx.channel())
            .filter(|ch| ch.commands.contains_key(&*cmd))
            .is_some()
        {
            "updated"
        } else {
            "added"
        };

        let msg = &ctx.args.msg;
        let responder = &ctx.responder();
        self.update_template(msg, responder, cmd, body, action)
            .await
    }

    async fn add_command(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let cmd = Self::get_command(&ctx);
        let body = ctx.args.get_non_empty("body");

        if let Some(ch) = self.channels.lock().await.get(ctx.channel()) {
            if ch.commands.contains_key(&*cmd) {
                return ctx.reply(format!("'{}' already exists", cmd));
            }
        }

        let msg = &ctx.args.msg;
        let responder = &ctx.responder();
        self.update_template(msg, responder, cmd, body, "added")
            .await
    }

    async fn remove_command(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let cmd = Self::get_command(&ctx);

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

        self.sync_commands().await?;

        ctx.reply(out)
    }

    fn get_command(ctx: &Context<CommandArgs>) -> &str {
        ctx.args["command"].trim_start_matches(Command::LEADER)
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

    async fn update_template(
        &self,
        msg: &Privmsg<'_>,
        responder: &Responder,
        cmd: &str,
        body: Option<&str>,
        action: &str,
    ) -> anyhow::Result<()> {
        let body = match body.map(str::trim).filter(|s| !s.is_empty()) {
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

        responder.reply(msg, format!("{} '{}' -> '{}'", action, cmd, body))?;

        self.channels
            .lock()
            .await
            .entry(msg.channel().to_string())
            .or_default()
            .add_template(cmd, SimpleTemplate::new(cmd, body));

        self.sync_commands().await
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

        Toml::save(&self.config.commands_file, &saved)
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

        let commands = commands.collect();
        Self { commands }
    }
}

mod data {
    use crate::persist::{Persist, Toml};
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
        Toml::load_from(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestRunner;

    // let s = std::fs::read_to_string(&temp).unwrap();
    // eprintln!("{}", s)

    #[test]
    fn cannot_do_it() {
        let commands = &[
            "!add foo bar", //
            "!set foo bar",
            "!remove foo",
        ];

        for command in commands {
            TestRunner::new(*command)
                .reply("you cannot do that")
                .with_module(Responses::initialize)
                .run_commands(|| {});
        }
    }

    #[test]
    fn add_broadcaster() {
        let temp = tempfile::Builder::new().tempfile().unwrap();

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!add hello world")
            .with_broadcaster("museun")
            .reply("added 'hello' -> 'world'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});
    }

    #[test]
    fn add_broadcaster_twice() {
        let temp = tempfile::Builder::new().tempfile().unwrap();

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!add hello world")
            .with_broadcaster("museun")
            .reply("added 'hello' -> 'world'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!add hello world")
            .with_broadcaster("museun")
            .reply("'hello' already exists")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});
    }

    #[test]
    fn add_moderator() {
        let temp = tempfile::Builder::new().tempfile().unwrap();

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!add hello world")
            .with_moderator("museun")
            .reply("added 'hello' -> 'world'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});
    }

    #[test]
    fn add_moderator_twice() {
        let temp = tempfile::Builder::new().tempfile().unwrap();

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!add hello world")
            .with_moderator("museun")
            .reply("added 'hello' -> 'world'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!add hello world")
            .with_moderator("museun")
            .reply("'hello' already exists")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});
    }

    #[test]
    fn set() {
        let temp = tempfile::Builder::new().tempfile().unwrap();

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!set foo bar")
            .with_broadcaster("museun")
            .reply("added 'foo' -> 'bar'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});
    }

    #[test]
    fn set_twice() {
        let temp = tempfile::Builder::new().tempfile().unwrap();

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!set foo bar")
            .with_broadcaster("museun")
            .reply("added 'foo' -> 'bar'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!set foo bar")
            .with_broadcaster("museun")
            .reply("updated 'foo' -> 'bar'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});

        let commands_file = temp.path().display().to_string();
        TestRunner::new("!set foo bar")
            .with_broadcaster("museun")
            .reply("updated 'foo' -> 'bar'")
            .config(|config| config.modules.commands.commands_file = commands_file)
            .with_module(Responses::initialize)
            .run_commands(|| {});
    }

    #[test]
    #[ignore]
    fn remove() {
        todo!()
    }

    #[test]
    #[ignore]
    fn call() {}

    #[test]
    #[ignore]
    fn call_missing() {}
}
