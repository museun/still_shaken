use super::{command::ExtractResult, handler::AnyhowFut, Callable, Command, Respond};
use crate::{util::PrivmsgExt, Context};

use std::{collections::HashMap, future::Future, sync::Arc};
use twitchchat::messages::Privmsg;

#[derive(Clone)]
pub struct CommandArgs {
    pub cmd: Arc<Command>,
    pub msg: Arc<Privmsg<'static>>,
    pub map: HashMap<Box<str>, Box<str>>, // this is lame
}

impl std::ops::Index<&str> for CommandArgs {
    type Output = str;
    fn index(&self, index: &str) -> &Self::Output {
        &self.map[index]
    }
}

impl CommandArgs {
    pub fn get_parsed<K, T>(&self, key: &K) -> anyhow::Result<T>
    where
        Box<str>: std::borrow::Borrow<K>,
        K: Eq + std::hash::Hash + ?Sized,
        K: std::fmt::Display,
        T: std::str::FromStr,
        T::Err: Into<anyhow::Error> + Send + 'static,
    {
        use anyhow::Context as _;
        let t = self
            .map
            .get(key)
            .map(|s| s.parse().map_err(Into::into))
            .with_context(|| anyhow::anyhow!("cannot lookup: {}", key))??;
        Ok(t)
    }

    pub fn get_non_empty<K>(&self, key: &K) -> Option<&str>
    where
        Box<str>: std::borrow::Borrow<K>,
        K: Eq + std::hash::Hash + ?Sized,
    {
        self.map.get(key).filter(|c| !c.is_empty()).map(|s| &**s)
    }

    pub fn contains<K>(&self, key: &K) -> bool
    where
        Box<str>: std::borrow::Borrow<K>,
        K: Eq + std::hash::Hash + ?Sized,
    {
        self.map.contains_key(key)
    }
}

pub struct StoredCommand {
    cmd: Command,
    callable: Box<dyn Fn(Context<CommandArgs>) -> AnyhowFut<'static> + Send>,
}

impl StoredCommand {
    pub fn new<T, Fut>(
        this: Arc<T>,
        example: &str,
        func: impl Fn(Arc<T>, Context<CommandArgs>) -> Fut + Send + Sync + 'static,
    ) -> anyhow::Result<Self>
    where
        T: Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
        Fut: Send + Sync + 'static,
    {
        Self::build_with(this, example, |cmd| cmd, func)
    }

    pub fn elevated<T, Fut>(
        this: Arc<T>,
        example: &str,
        func: impl Fn(Arc<T>, Context<CommandArgs>) -> Fut + Send + Sync + 'static,
    ) -> anyhow::Result<Self>
    where
        T: Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
        Fut: Send + Sync + 'static,
    {
        Self::build_with(this, example, |cmd| cmd.elevated(), func)
    }

    pub fn build_with<T, Fut, F>(
        this: Arc<T>,
        example: &str,
        map: F,
        func: impl Fn(Arc<T>, Context<CommandArgs>) -> Fut + Send + Sync + 'static,
    ) -> anyhow::Result<Self>
    where
        T: Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
        Fut: Send + Sync + 'static,
        F: Fn(Command) -> Command,
    {
        map(Command::example(example)).build().map(|cmd| Self {
            callable: Box::new(move |ctx| Box::pin(func(this.clone(), ctx))),
            cmd,
        })
    }
}

impl Callable<CommandArgs> for StoredCommand {
    type Fut = AnyhowFut<'static>;
    fn call(&self, state: Context<CommandArgs>) -> Self::Fut {
        (self.callable)(state)
    }
}

#[derive(Default)]
pub struct Commands {
    commands: HashMap<Arc<Command>, Box<dyn Callable<CommandArgs, Fut = AnyhowFut<'static>>>>,
}

impl Commands {
    pub fn add(
        &mut self,
        cmd: Command,
        callable: impl Callable<CommandArgs, Fut = AnyhowFut<'static>>,
    ) -> anyhow::Result<()> {
        // TODO assert about overridden commands
        self.commands.insert(Arc::new(cmd), Box::new(callable));
        Ok(())
    }

    pub fn command<T, Fut>(
        &mut self,
        this: Arc<T>,
        example: &str,
        func: impl Fn(Arc<T>, Context<CommandArgs>) -> Fut + Send + Sync + 'static,
    ) -> anyhow::Result<()>
    where
        T: Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
        Fut: Send + Sync + 'static,
    {
        self.add_stored(StoredCommand::new(this, example, func)?)
    }

    pub fn elevated<T, Fut>(
        &mut self,
        this: Arc<T>,
        example: &str,
        func: impl Fn(Arc<T>, Context<CommandArgs>) -> Fut + Send + Sync + 'static,
    ) -> anyhow::Result<()>
    where
        T: Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
        Fut: Send + Sync + 'static,
    {
        self.add_stored(StoredCommand::elevated(this, example, func)?)
    }

    pub fn add_stored(&mut self, mut stored: StoredCommand) -> anyhow::Result<()> {
        // TODO assert about overridden commands
        let cmd = Arc::new(std::mem::take(&mut stored.cmd));
        self.commands.insert(cmd, Box::new(stored));
        Ok(())
    }

    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.keys().map(|s| &**s)
    }
}

impl Callable<Privmsg<'static>> for Commands {
    type Fut = AnyhowFut<'static>;

    fn call(&self, state: Context<Privmsg<'static>>) -> Self::Fut {
        for (k, v) in &self.commands {
            // we should have unique commands
            let map = match k.extract(state.args.data()) {
                ExtractResult::Found(map) => map,
                ExtractResult::Required => {
                    let _ = state.reply(k.help());
                    continue;
                }
                ExtractResult::NoMatch => continue,
            };

            if k.requires_elevated() && !state.args.is_above_user_level() {
                return Box::pin(async move { state.reply("you cannot do that") });
            }

            let map = map.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
            let args = CommandArgs {
                cmd: k.clone(),
                msg: state.args.clone(),
                map,
            };
            let ctx = Context::new(args, state.state.clone());
            return Box::pin(v.call(ctx));
        }

        Box::pin(async move { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use crate::bot::test::TestRunner;

    use super::*;
    #[test]
    fn command_thing() {
        fn make_commands() -> Commands {
            let hello = Command::example("!hello").build().unwrap();
            let repeat_this = Command::example("!repeat <this...>").build().unwrap();
            let maybe = Command::example("!maybe <something?>").build().unwrap();
            let elevated = Command::example("!shutdown").elevated().build().unwrap();

            let mut dispatch = Commands::default();
            dispatch
                .add(hello, |ctx: Context<CommandArgs>| async move {
                    ctx.say(format!("hello {}", ctx.args.msg.name()))
                })
                .unwrap();
            dispatch
                .add(repeat_this, |ctx: Context<CommandArgs>| async move {
                    ctx.say(format!("ok: {}", ctx.args.map["this"]))
                })
                .unwrap();
            dispatch
                .add(maybe, |ctx: Context<CommandArgs>| async move {
                    match ctx.args.map.get("something") {
                        Some(data) => ctx.say(format!("just: {}", data)),
                        None => ctx.say("nothing"),
                    }
                })
                .unwrap();
            dispatch
                .add(elevated, |ctx: Context<CommandArgs>| async move {
                    ctx.reply("shutting down")
                })
                .unwrap();

            dispatch
        }

        let commands = make_commands();

        let commands = TestRunner::new("!hello")
            .say("hello test_user")
            .run(commands);

        let commands = TestRunner::new("!hello world")
            .say("hello test_user")
            .run(commands);

        let commands = TestRunner::new("!repeat some message")
            .say("ok: some message")
            .run(commands);

        let commands = TestRunner::new("!repeat something")
            .say("ok: something")
            .run(commands);

        let commands = TestRunner::new("!maybe monad")
            .say("just: monad")
            .run(commands);

        let commands = TestRunner::new("!maybe").say("nothing").run(commands);

        let commands = TestRunner::new("!shutdown")
            .with_broadcaster("museun")
            .reply("shutting down")
            .run(commands);

        let _commands = TestRunner::new("!shutdown")
            .reply("you cannot do that")
            .run(commands);
    }
}
