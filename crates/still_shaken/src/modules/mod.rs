use crate::*;

mod commands;
mod crates;
mod shaken;

mod help;

pub fn initialize_modules(
    config: &Config,
    commands: &mut CommandDispatch,
    passives: &mut Passives,
    executor: &Executor,
) -> anyhow::Result<()> {
    crates::initialize(config, commands, passives, executor)?;
    commands::initialize(config, commands, passives, executor)?;
    shaken::initialize(config, commands, passives, executor)?;

    // this has to be last
    help::initialize(config, commands, passives, executor)?;
    Ok(())
}
