use super::*;
use rand::Rng;
use twitchchat::runner::Identity;

mod shaken;
use shaken::Shaken;

mod commands;
use commands::Commands;

mod crates;

pub fn create_tasks<R>(
    config: &Config, //
    responder: Responder,
    identity: Identity,
    executor: Executor,
    rng: R,
) -> Tasks
where
    R: Rng + Send + Sync + 'static + Clone,
{
    Tasks::new(responder, identity, executor)
        .with(Shaken::new(&config.modules.shaken, rng))
        .with(Commands::new(&config.modules.commands))
        .with(crates::lookup_crate)
}
