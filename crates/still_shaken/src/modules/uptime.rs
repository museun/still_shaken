use std::sync::Arc;
use std::time::Instant;

use crate::*;

pub struct Uptime(Instant);

impl Uptime {
    fn new() -> Arc<Self> {
        Arc::new(Self(Instant::now()))
    }
}

impl super::Initialize for Uptime {
    fn initialize(
        _config: &crate::Config,
        commands: &mut crate::Commands,
        _passives: &mut crate::Passives,
        _executor: &crate::Executor,
    ) -> anyhow::Result<()> {
        let handle = |this: Arc<Self>, ctx: Context<CommandArgs>| async move {
            ctx.say(format!(
                "I've been running for {}",
                this.0.elapsed().relative_time()
            ))
        };

        commands.command(Self::new(), "!uptime", handle)
    }
}
