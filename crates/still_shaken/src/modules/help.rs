use crate::*;
use std::{borrow::Cow, sync::Arc};

pub struct Help;
impl super::Initialize for Help {
    fn initialize(
        config: &Config,
        commands: &mut Commands,
        _passives: &mut Passives,
        _executor: &Executor,
    ) -> anyhow::Result<()> {
        let cmd = Command::example("!help <command?>").build()?;
        let help = HelpCommand {
            commands: Arc::new({
                let mut cmds = vec![cmd.clone()];
                cmds.extend(commands.commands().cloned());
                cmds
            }),
            config: Arc::new(config.clone()),
        };
        commands.add(cmd, move |ctx| help.call(ctx))?;

        Ok(())
    }
}

#[derive(Clone)]
struct HelpCommand {
    commands: Arc<Vec<Command>>,
    config: Arc<Config>,
}

impl Callable<CommandArgs> for HelpCommand {
    type Fut = AnyhowFut<'static>;
    fn call(&self, state: Context<CommandArgs>) -> Self::Fut {
        let fut = Self::handle(Box::new(self.clone()), state);
        Box::pin(fut)
    }
}

impl HelpCommand {
    async fn handle(self: Box<Self>, context: Context<CommandArgs>) -> anyhow::Result<()> {
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
        let custom = super::get_commands(&*self.config, channel)?;

        let commands = self
            .commands
            .iter()
            .map(|d| d.name())
            .chain(custom.iter().map(|(k, _)| &**k))
            .fold(String::new(), |mut a, c| {
                if !a.is_empty() {
                    a.push_str(", ");
                }
                a.push_str(Command::LEADER);
                a.push_str(c);
                a
            });

        Ok(commands)
    }

    fn lookup(&self, cmd: &str, channel: &str) -> anyhow::Result<Cow<'_, str>> {
        let search = cmd.trim_start_matches(Command::LEADER);
        match self.commands.iter().find(|c| c.name() == search) {
            Some(cmd) => Ok(cmd.help().into()),
            None => {
                match super::get_commands(&*self.config, channel)?
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
