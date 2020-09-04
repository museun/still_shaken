use super::{Context, Executor, Handler};
use crate::{config, util::PrivmsgExt as _};

use async_executor::Task;
use futures_lite::StreamExt as _;

use twitchchat::messages::Privmsg;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub struct Shaken {
    timeout: Duration,
    generate: Arc<String>,
    config: config::Shaken,
    last: Option<Instant>,
    rng: fastrand::Rng,
}

impl Shaken {
    pub fn new(config: &config::Shaken, rng: fastrand::Rng) -> Self {
        Self {
            timeout: Duration::from_millis(config.timeout),
            generate: Arc::new(format!("{}/generate", config.host)),
            config: config.clone(),
            last: None,
            rng,
        }
    }
}

impl Handler for Shaken {
    fn spawn(mut self, mut context: Context, executor: Executor) -> Task<()> {
        let fut = async move {
            while let Some(msg) = context.stream.next().await {
                if let Err(err) = self.handle(&*msg, &mut context).await {
                    log::error!("cannot do the shaken thing: {}", err);
                }
            }
        };
        executor.spawn(fut)
    }
}

impl Shaken {
    async fn handle(
        &mut self,
        msg: &Privmsg<'static>,
        context: &mut Context,
    ) -> anyhow::Result<()> {
        if msg.data() == "!speak" || msg.is_mentioned(&*context.identity) {
            let response = Self::fetch_response(&*self.generate, None).await?;
            let response = fixup_response(response);
            let _ = context.responder.say(msg, response);
            return Ok(());
        }

        if let Some(data) = self.generate(msg.data()).await? {
            let _ = context.responder.say(msg, data);
        }

        Ok(())
    }

    async fn generate(&mut self, context: &str) -> anyhow::Result<Option<String>> {
        if let Some(dur) = self.last {
            if dur.elapsed() < self.timeout || self.rng.f64() <= self.config.ignore_chance {
                return Ok(None);
            }
        }

        let context = self.choose_context(context).map(ToString::to_string);
        let response = Self::fetch_response(&*self.generate, context).await?;
        let response = fixup_response(response);

        // random delay
        self.random_delay().await;

        // and then update out last spoken marker
        self.last.replace(Instant::now());

        log::trace!("generated '{}'", response.escape_debug());
        Ok(Some(response))
    }

    async fn random_delay(&mut self) {
        let lower = std::cmp::max(self.config.delay_lower, self.config.delay_upper / 10);
        let upper = self.rng.u64(lower..self.config.delay_upper);
        let range = self.rng.u64(self.config.delay_lower..upper);
        let delay = Duration::from_millis(range);
        async_io::Timer::after(delay).await;
    }

    fn choose_context<'a>(&mut self, context: &'a str) -> Option<&'a str> {
        let mut choices = context
            .split_whitespace()
            .filter(|&s| filtered_context(s))
            .collect::<Vec<_>>();
        self.rng.shuffle(&mut choices);
        choices.get(0).copied()
    }

    async fn fetch_response(host: &str, context: Option<String>) -> anyhow::Result<String> {
        #[derive(Debug, serde::Deserialize)]
        struct Response {
            status: String,
            data: String,
        }

        let body = serde_json::json!({
            "min": 1,
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
