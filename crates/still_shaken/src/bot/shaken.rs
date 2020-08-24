use super::{Context, Handler, PrivmsgExt};

use futures_lite::StreamExt as _;
use rand::{prelude::*, Rng};

use crate::config;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use twitchchat::messages::Privmsg;

pub struct Shaken<R> {
    timeout: Duration,
    generate: Arc<String>,
    config: config::Shaken,
    last: Option<Instant>,
    rng: R,
}

impl<R> Shaken<R> {
    pub fn new(config: &config::Shaken, rng: R) -> Self
    where
        R: Rng + Send + Sync + 'static,
    {
        Self {
            timeout: Duration::from_millis(config.timeout),
            generate: Arc::new(format!("{}/generate", config.host)),
            config: config.clone(),
            last: None,
            rng,
        }
    }
}

impl<R> Handler for Shaken<R>
where
    R: Rng + Send + Sync + 'static,
{
    fn spawn(mut self, mut context: Context) -> smol::Task<()> {
        let fut = async move {
            while let Some(msg) = context.stream.next().await {
                if let Err(err) = self.handle(&*msg, &mut context).await {
                    log::error!("cannot do the shaken thing: {}", err);
                }
            }
        };
        smol::Task::spawn(fut)
    }
}

impl<R> Shaken<R>
where
    R: Rng + Send + Sync + 'static,
{
    async fn handle(
        &mut self,
        msg: &Privmsg<'static>,
        context: &mut Context,
    ) -> anyhow::Result<()> {
        if msg.data() == "!speak" || msg.is_mentioned(&*context.identity) {
            let response = Self::fetch_response(self.generate.clone(), None).await?;
            let response = fixup_response(response);
            let _ = context.responder.say(&msg, response);
            return Ok(());
        }

        if let Some(data) = self.generate(msg.data()).await? {
            let _ = context.responder.say(&msg, data);
        }

        Ok(())
    }

    async fn generate(&mut self, context: &str) -> anyhow::Result<Option<String>> {
        if let Some(dur) = self.last {
            if dur.elapsed() < self.timeout || !self.rng.gen_bool(self.config.ignore_chance) {
                return Ok(None);
            }
        }

        let context = self.choose_context(context).map(ToString::to_string);
        let response = Self::fetch_response(self.generate.clone(), context).await?;
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
        let upper = self.rng.gen_range(lower, self.config.delay_upper);
        let range = self.rng.gen_range(self.config.delay_lower, upper);
        let delay = Duration::from_millis(range);
        smol::Timer::new(delay).await;
    }

    fn choose_context<'a>(&mut self, context: &'a str) -> Option<&'a str> {
        context
            .split_whitespace()
            .filter(|&s| filtered_context(s))
            .choose(&mut self.rng)
    }

    async fn fetch_response(host: Arc<String>, context: Option<String>) -> anyhow::Result<String> {
        const MIN: usize = 1;
        const MAX: usize = 45;
        const MIN_LEN: usize = 1;

        smol::unblock!({
            #[derive(Debug, serde::Deserialize)]
            struct Response {
                status: String,
                data: String,
            }

            let data = loop {
                let response = attohttpc::get(&*host)
                    .json(&serde_json::json!({
                        "min": MIN,
                        "max": MAX,
                        "context": &context
                    }))?
                    .send()?
                    .json::<Response>()?;

                if response.data.len() > MIN_LEN {
                    break response.data;
                }
            };

            anyhow::Result::<_, anyhow::Error>::Ok(data)
        })
    }
}

fn filtered_context(s: &str) -> bool {
    !s.starts_with("http") && !s.starts_with('!') && !s.starts_with('.')
}

fn fixup_response(response: String) -> String {
    "~ ".to_string() + &response
}
