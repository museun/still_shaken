use crate::*;

use async_mutex::Mutex;
use error::DontCare;

use twitchchat::messages::Privmsg;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub struct Shaken {
    timeout: Duration,
    generate: Arc<String>,
    config: config::Shaken,
    last: Mutex<Option<Instant>>,
}

impl super::Initialize for Shaken {
    fn initialize(
        config: &Config,
        commands: &mut Commands,
        passives: &mut Passives,
        _executor: &Executor,
    ) -> anyhow::Result<()> {
        let this = Arc::new(Self::new(&config.modules.shaken));

        commands.command(this.clone(), "!speak", Self::speak)?;
        passives.with(this, Self::handle);

        Ok(())
    }
}

impl Shaken {
    pub fn new(config: &config::Shaken) -> Self {
        Self {
            timeout: Duration::from_millis(config.timeout),
            generate: Arc::new(format!("{}/generate", config.host)),
            config: config.clone(),
            last: Default::default(),
        }
    }
}

impl Shaken {
    async fn speak(self: Arc<Self>, ctx: Context<CommandArgs>) -> anyhow::Result<()> {
        let response = Self::fetch_response(&*self.generate, None).await?;
        let response = fixup_response(response);
        ctx.say(response)
    }

    async fn handle(self: Arc<Self>, ctx: Context<Privmsg<'static>>) -> anyhow::Result<()> {
        if ctx.args.is_mentioned(&*ctx.state.identity) {
            let response = Self::fetch_response(&*self.generate, None).await?;
            let response = fixup_response(response);
            return ctx.say(response);
        }

        // let everything else run before this
        async_io::Timer::after(std::time::Duration::from_secs(1)).await;
        let data = self.generate(ctx.args.data()).await?.dont_care()?;
        ctx.say(data)
    }

    async fn generate(self: Arc<Self>, context: &str) -> anyhow::Result<Option<String>> {
        if let Some(dur) = &*self.last.lock().await {
            if dur.elapsed() < self.timeout || fastrand::f64() <= self.config.ignore_chance {
                return Ok(None);
            }
        }

        let context = self.choose_context(context).map(ToString::to_string);

        let response = Self::fetch_response(&*self.generate, context).await?;
        let response = fixup_response(response);

        // random delay
        self.random_delay().await;

        // and then update out last spoken marker
        self.last.lock().await.replace(Instant::now());

        log::trace!("generated '{}'", response.escape_debug());
        Ok(Some(response))
    }

    async fn random_delay(&self) {
        let lower = std::cmp::max(self.config.delay_lower, self.config.delay_upper / 10);
        let upper = fastrand::u64(lower..self.config.delay_upper);
        let range = fastrand::u64(self.config.delay_lower..upper);
        let delay = Duration::from_millis(range);
        async_io::Timer::after(delay).await;
    }

    fn choose_context<'a>(&self, context: &'a str) -> Option<&'a str> {
        let mut choices = context
            .split_whitespace()
            .filter(|&s| filtered_context(s))
            .collect::<Vec<_>>();
        fastrand::shuffle(&mut choices);
        choices.get(0).copied()
    }

    async fn fetch_response(host: &str, context: Option<String>) -> anyhow::Result<String> {
        #[derive(Debug, serde::Deserialize)]
        struct Response {
            status: String,
            data: String,
        }

        let body = serde_json::json!({
            "min": fastrand::u8(1..=3),
            "max": 45,
            "context": &context
        });

        crate::http::get_json_with_body(host, body)
            .await
            .map(|resp: Response| resp.data)
    }
}

fn filtered_context(s: &str) -> bool {
    !s.starts_with("http") && !s.starts_with('!') && !s.starts_with('.')
}

fn fixup_response(response: String) -> String {
    "~ ".to_string() + &response
}
