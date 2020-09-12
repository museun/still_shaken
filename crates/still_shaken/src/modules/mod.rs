use crate::*;

macro_rules! import {
    ($($ident:ident)*) => {
        $(
            mod $ident; use $ident::*;
        )*
    };
}

import! {
    responses
    crates
    shaken
    help
}

pub fn initialize_modules(
    config: &Config,
    commands: &mut Commands,
    passives: &mut Passives,
    executor: &Executor,
) -> anyhow::Result<()> {
    Crates::initialize(config, commands, passives, executor)?;
    Responses::initialize(config, commands, passives, executor)?;
    Shaken::initialize(config, commands, passives, executor)?;

    // this has to be last
    Help::initialize(config, commands, passives, executor)?;
    Ok(())
}

trait Initialize {
    fn initialize(
        config: &Config,
        commands: &mut Commands,
        passives: &mut Passives,
        executor: &Executor,
    ) -> anyhow::Result<()>;
}
