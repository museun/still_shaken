use modules::Components;

use crate::*;
use std::{borrow::Cow, sync::Arc};

pub struct Help {
    commands: Vec<ShakenCommand>,
    config: Config,
}

impl super::Initialize for Help {
    fn initialize(
        Components {
            config, commands, ..
        }: &mut Components<'_>,
    ) -> anyhow::Result<()> {
        // add the dummy help command show it shows up in itself
        let mut cmds = vec![shaken_commands::Command::example("!help <command?>")
            .build()?
            .into()];
        cmds.extend(commands.commands().cloned());

        // and this is the real command
        let this = Arc::new(Self::new(cmds, config.clone()));
        commands.command(this, "!help <command?>", Self::handle)?;

        Ok(())
    }
}

impl Help {
    const fn new(commands: Vec<ShakenCommand>, config: Config) -> Self {
        Self { commands, config }
    }

    async fn handle(self: Arc<Self>, context: Context<CommandArgs>) -> anyhow::Result<()> {
        let channel = context.channel();
        match context.args.map.get("command") {
            Some(cmd) => {
                let command = self.lookup(cmd, channel)?;
                context.reply(command)
            }
            None => {
                let commands = self.format_commands(channel)?;
                context.say(&*commands)
            }
        }
    }

    fn format_commands(&self, channel: &str) -> anyhow::Result<String> {
        let custom = super::get_commands(&self.config, channel)?;

        let commands = self
            .commands
            .iter()
            .map(|d| d.command())
            .chain(custom.iter().map(|(k, _)| &**k))
            .fold(String::new(), |mut a, c| {
                if !a.is_empty() {
                    a.push_str(", ");
                }
                a.push_str(shaken_commands::Command::LEADER);
                a.push_str(c);
                a
            });

        Ok(commands)
    }

    fn lookup(&self, cmd: &str, channel: &str) -> anyhow::Result<Cow<'_, str>> {
        let search = cmd.trim_start_matches(shaken_commands::Command::LEADER);
        match self.commands.iter().find(|c| c.command() == search) {
            Some(cmd) => Ok(cmd.help().into()),
            None => {
                match super::get_commands(&self.config, channel)?
                    .into_iter()
                    .find(|(k, _)| k == search)
                    .map(|(_, v)| v.into())
                {
                    Some(cmd) => Ok(cmd),
                    None => Ok(format!("I don't know what '{}' is", cmd).into()),
                }
            }
        }
    }
}
