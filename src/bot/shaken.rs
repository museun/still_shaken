use super::{Config, Handler, Recv, Responder};

use futures_lite::StreamExt as _;
use rand::prelude::*;

use std::time::{Duration, Instant};

pub struct Shaken<R> {
    timeout: Duration,
    host: String,

    delay_lower: u64,
    delay_upper: u64,
    ignore_chance: f64,

    last: Option<Instant>,
    rng: R,
}

impl<R> Shaken<R> {
    pub fn new(config: &Config, rng: R) -> Self
    where
        R: rand::Rng + Send + Sync + 'static,
    {
        let crate::config::Shaken {
            ref host,
            timeout,
            delay_lower,
            delay_upper,
            ignore_chance,
        } = config.modules.shaken;

        Self {
            timeout: Duration::from_millis(timeout),
            host: host.clone(),
            delay_lower,
            delay_upper,
            ignore_chance,

            last: None,
            rng,
        }
    }
}

impl<R> Handler for Shaken<R>
where
    R: rand::Rng + Send + Sync + 'static,
{
    fn sink(mut self, mut recv: Recv, responder: Responder) -> smol::Task<()> {
        smol::Task::spawn(async move {
            while let Some(msg) = recv.next().await {
                match self.generate(msg.data()).await {
                    Ok(Some(data)) => responder.say(&msg, data),
                    Err(..) => break,
                    _ => {}
                }
            }
        })
    }
}

impl<R> Shaken<R>
where
    R: rand::Rng + Send + Sync + 'static,
{
    async fn generate(&mut self, context: &str) -> anyhow::Result<Option<String>> {
        if let Some(dur) = self.last {
            if dur.elapsed() < self.timeout || !self.rng.gen_bool(self.ignore_chance) {
                return Ok(None);
            }
        }

        let context = self.choose_context(context).to_string();
        let host = format!("{}/generate", self.host);

        let response = Self::fetch_response(host, context).await?;
        let response = Self::fixup_response(response);

        // random delay
        self.rando_delay().await;

        // and then update out last spoken marker
        self.last.replace(Instant::now());

        log::warn!("generated '{}'", response.escape_debug());
        Ok(Some(response))
    }

    async fn rando_delay(&mut self) {
        let lower = std::cmp::max(self.delay_lower, self.delay_upper / 10);
        let upper = self.rng.gen_range(lower, self.delay_upper);
        let range = self.rng.gen_range(self.delay_lower, upper);
        let delay = Duration::from_millis(range);
        smol::Timer::new(delay).await;
    }

    fn filtered_context(s: &str) -> bool {
        !s.starts_with("http")
    }

    fn fixup_response(response: String) -> String {
        response
    }

    fn choose_context<'a>(&mut self, context: &'a str) -> &'a str {
        context
            .split_whitespace()
            .filter(|s| Self::filtered_context(s))
            .choose(&mut self.rng)
            .expect("context")
    }

    async fn fetch_response(host: String, context: String) -> anyhow::Result<String> {
        const MIN: usize = 1;
        const MAX: usize = 45;

        #[derive(Debug, serde::Deserialize)]
        struct Response {
            status: String,
            data: String,
        }

        smol::unblock!({
            let data = loop {
                let response = attohttpc::get(&host)
                    .json(&serde_json::json!({
                        "min": MIN,
                        "max": MAX,
                        "context": &context
                    }))?
                    .send()?
                    .json::<Response>()?;
                if response.data.len() > 1 {
                    break response.data;
                }
            };

            anyhow::Result::<_, anyhow::Error>::Ok(data)
        })
    }
}
