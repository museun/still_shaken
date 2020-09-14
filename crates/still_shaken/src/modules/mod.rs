use crate::*;

macro_rules! import {
    ($($ident:ident)*) => {
        $( mod $ident; use $ident::*; )*
    };
}

import! {
    crates
    help
    responses
    shaken
    uptime
}

struct Components<'a> {
    pub config: &'a Config,
    pub commands: &'a mut Commands,
    pub passives: &'a mut Passives,
    pub executor: &'a Executor,
}

trait Initialize {
    fn initialize(components: &mut Components<'_>) -> anyhow::Result<()>;
}

pub fn initialize_modules(
    config: &Config,
    commands: &mut Commands,
    passives: &mut Passives,
    executor: &Executor,
) -> anyhow::Result<()> {
    let components = &mut Components {
        config,
        commands,
        passives,
        executor,
    };

    Crates::initialize(components)?;
    Responses::initialize(components)?;
    Shaken::initialize(components)?;
    Uptime::initialize(components)?;

    // this has to be last
    Help::initialize(components)?;
    Ok(())
}
