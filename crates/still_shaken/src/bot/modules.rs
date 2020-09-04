use super::*;

use twitchchat::runner::Identity;

mod shaken;
use shaken::Shaken;

mod commands;
use commands::Commands;

mod crates;

pub fn create_tasks(
    config: &Config, //
    responder: Responder,
    identity: Identity,
    executor: Executor,
    rng: fastrand::Rng,
) -> Tasks {
    Tasks::new(responder, identity, executor)
        .with(Shaken::new(&config.modules.shaken, rng))
        .with(Commands::new(&config.modules.commands))
        .with(crates::lookup_crate)
}
