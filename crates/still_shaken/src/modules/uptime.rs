#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;

use std::sync::Arc;

use super::{Components, Initialize};
use crate::*;

pub struct Uptime(Instant);

impl Uptime {
    fn new() -> Arc<Self> {
        Arc::new(Self(Instant::now()))
    }
}

impl Initialize for Uptime {
    fn initialize(Components { commands, .. }: &mut Components<'_>) -> anyhow::Result<()> {
        let handle = |this: Arc<Self>, ctx: Context<CommandArgs>| async move {
            ctx.say(format!(
                "I've been running for {}.",
                this.0.elapsed().relative_time()
            ))
        };

        commands.command(Self::new(), "!uptime", handle)
    }
}

#[cfg(test)]
mod tests {
    use mock_instant::MockClock;
    use std::time::Duration;

    use super::*;
    use crate::TestRunner;

    #[test]
    fn uptime() {
        TestRunner::new("!uptime")
            .say("I've been running for 1 minute and 1 second.")
            .with_module(Uptime::initialize)
            .run_commands(|| MockClock::advance(Duration::from_secs(61)))
    }
}
