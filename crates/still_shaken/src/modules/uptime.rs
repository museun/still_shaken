use crate::format::FormatTime;
use std::sync::Arc;

use crate::*;

pub struct Uptime {
    start: std::time::Instant,
}

impl super::Initialize for Uptime {
    fn initialize(
        _config: &crate::Config,
        commands: &mut crate::Commands,
        _passives: &mut crate::Passives,
        _executor: &crate::Executor,
    ) -> anyhow::Result<()> {
        commands.command(
            Arc::new(Self::new()), //
            "!uptime",
            Uptime::uptime,
        )
    }
}

impl Uptime {
    fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    async fn uptime(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        ctx.say(format!(
            "I've been running for {}",
            self.start.elapsed().relative_time()
        ))
    }
}
