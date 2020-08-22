use super::{Config, Responder, Response, Shaken, Tasks, Writer};

use futures_lite::StreamExt;
use twitchchat::{messages::Commands, Status};

pub struct Runner {
    config: Config,
    runner: twitchchat::AsyncRunner,
}

impl Runner {
    pub async fn connect(config: Config) -> anyhow::Result<Self> {
        let (name, token) = (&config.identity.name, Self::get_token()?);

        let connector = twitchchat::connector::SmolConnector::twitch();
        let user_config = twitchchat::UserConfig::builder()
            .name(name)
            .token(token)
            .enable_all_capabilities()
            .build()?;

        twitchchat::AsyncRunner::connect(connector, &user_config)
            .await
            .map_err(Into::into)
            .map(|runner| Self { config, runner })
    }

    pub async fn join_channel(&mut self) -> anyhow::Result<()> {
        for channel in &self.config.identity.channels {
            match self.runner.join(channel).await {
                Err(twitchchat::RunnerError::BannedFromChannel { channel }) => {
                    log::error!("cannot join '{}'. we're banned", channel);
                    continue;
                }
                Err(err) => return Err(err.into()),
                Ok(..) => {}
            }
        }

        Ok(())
    }

    pub async fn run_to_completion<R>(mut self, rng: R) -> anyhow::Result<()>
    where
        R: rand::Rng + Send + Sync + 'static + Clone,
    {
        let responder = Self::create_responder(self.runner.writer());
        let mut tasks = Self::create_tasks(&self.config, responder, rng);

        loop {
            match self.runner.next_message().await? {
                Status::Message(Commands::Privmsg(msg)) => {
                    tasks.send_all(msg);
                }

                Status::Quit => break,

                // TODO reconnect if EOF
                Status::Eof => break,
                _ => continue,
            }
        }

        tasks.cancel_remaining().await;

        Ok(())
    }

    fn get_token() -> anyhow::Result<String> {
        const OAUTH_ENV_VAR: &str = "SHAKEN_TWITCH_OAUTH_TOKEN";

        std::env::var(OAUTH_ENV_VAR).map_err(|_| {
            anyhow::anyhow!(
                "please set `{}` to your associated Twitch OAuth token",
                OAUTH_ENV_VAR
            )
        })
    }

    fn create_tasks<R>(config: &Config, responder: Responder, rng: R) -> Tasks
    where
        R: rand::Rng + Send + Sync + 'static + Clone,
    {
        let mut tasks = Tasks::new(responder);

        let shaken = Shaken::new(&config, rng);
        tasks.spawn(shaken);

        tasks
    }

    fn create_responder(mut writer: Writer) -> Responder {
        let (tx, mut rx) = async_channel::bounded::<Response>(32);

        smol::Task::spawn(async move {
            while let Some(resp) = rx.next().await {
                if let Err(..) = writer.encode(resp).await {
                    // do something here
                }
            }
        })
        .detach();

        Responder::new(tx)
    }
}
