use super::{command::ExtractResult, handler::AnyhowFut, Callable, Command};
use crate::Context;

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
pub struct CommandDispatch {
    commands: HashMap<Arc<Command>, Box<dyn Callable<CommandArgs, Fut = AnyhowFut<'static>>>>,
}

impl CommandDispatch {
    pub fn add(
        &mut self,
        cmd: Command,
        callable: impl Callable<CommandArgs, Fut = AnyhowFut<'static>>,
    ) -> anyhow::Result<()> {
        // TODO assert about overridden commands
        self.commands.insert(Arc::new(cmd), Box::new(callable));
        Ok(())
    }

    pub fn add_many_stored(
        &mut self,
        iter: impl IntoIterator<Item = StoredCommand>,
    ) -> anyhow::Result<()> {
        iter.into_iter()
            .map(|stored| self.add_stored(stored))
            .collect()
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

impl Callable<Privmsg<'static>> for CommandDispatch {
    type Fut = AnyhowFut<'static>;

    fn call(&self, state: Context<Privmsg<'static>>) -> Self::Fut {
        for (k, v) in &self.commands {
            // we should have unique commands
            let map = match k.extract(state.args.data()) {
                ExtractResult::Found(map) => map,
                ExtractResult::Required => {
                    let _ = state.responder().reply(&*state.args, k.help());
                    continue;
                }
                ExtractResult::NoMatch => continue,
            };

            if !k.is_level_met(&*state.args) {
                return Box::pin(async move {
                    state.responder().reply(&*state.args, "you cannot do that")
                });
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
        fn make_commands() -> CommandDispatch {
            let hello = Command::example("!hello").build().unwrap();
            let repeat_this = Command::example("!repeat <this...>").build().unwrap();
            let maybe = Command::example("!maybe <something?>").build().unwrap();
            let elevated = Command::example("!shutdown").elevated().build().unwrap();

            let mut dispatch = CommandDispatch::default();
            dispatch
                .add(hello, |ctx: Context<CommandArgs>| async move {
                    ctx.responder()
                        .say(&*ctx.args.msg, format!("hello {}", ctx.args.msg.name()))
                })
                .unwrap();
            dispatch
                .add(repeat_this, |ctx: Context<CommandArgs>| async move {
                    ctx.responder()
                        .say(&*ctx.args.msg, format!("ok: {}", ctx.args.map["this"]))
                })
                .unwrap();
            dispatch
                .add(maybe, |ctx: Context<CommandArgs>| async move {
                    match ctx.args.map.get("something") {
                        Some(data) => ctx
                            .responder()
                            .say(&*ctx.args.msg, format!("just: {}", data)),
                        None => ctx.responder().say(&*ctx.args.msg, "nothing"),
                    }
                })
                .unwrap();
            dispatch
                .add(elevated, |ctx: Context<CommandArgs>| async move {
                    ctx.responder().reply(&*ctx.args.msg, "shutting down")
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
