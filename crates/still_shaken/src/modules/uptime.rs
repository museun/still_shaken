use std::sync::Arc;
use std::time::Instant;

use crate::*;
use modules::Components;

pub struct Uptime(Instant);

impl Uptime {
    fn new() -> Arc<Self> {
        Arc::new(Self(Instant::now()))
    }
}

impl super::Initialize for Uptime {
    fn initialize(Components { commands, .. }: &mut Components<'_>) -> anyhow::Result<()> {
        let handle = |this: Arc<Self>, ctx: Context<CommandArgs>| async move {
            ctx.say(format!(
                "I've been running for {}",
                this.0.elapsed().relative_time()
            ))
        };

        commands.command(Self::new(), "!uptime", handle)
    }
}
