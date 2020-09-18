use crate::error::DontCareSigil;

use super::{
    handler::{AnyhowFut, Callable, Context},
    Config, Executor, Responder, Response, State,
};

use async_mutex::Mutex;
use futures_lite::StreamExt;
use std::sync::Arc;
use twitchchat::{messages::Commands as TwitchCommands, messages::Privmsg, Status};

pub type ActiveCallable = dyn Callable<Privmsg<'static>, Fut = AnyhowFut<'static>>;

pub struct Runner {
    config: Config,
    runner: twitchchat::AsyncRunner,
    state: Arc<Mutex<State>>,
}

impl Runner {
    pub async fn connect(config: Config) -> anyhow::Result<Self> {
        let (name, token) = (&config.identity.name, Self::get_token()?);

        let connector = twitchchat::connector::AsyncIoConnectorTls::twitch();
        let user_config = twitchchat::UserConfig::builder()
            .name(name)
            .token(token)
            .enable_all_capabilities()
            .build()?;

        log::info!("connecting to Twitch...");
        twitchchat::AsyncRunner::connect(connector, &user_config)
            .await
            .map_err(Into::into)
            .map(|runner| Self {
                config,
                runner,
                state: <_>::default(),
            })
    }

    pub async fn join_channels(&mut self) -> anyhow::Result<()> {
        for channel in &self.config.identity.channels {
            log::info!("joining '{}'", channel);
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

    pub async fn run_to_completion(
        mut self,
        actives: &[Box<ActiveCallable>],
        executor: Executor,
    ) -> anyhow::Result<()> {
        let responder = Self::create_responder(self.runner.writer(), &executor);
        let identity = Arc::new(self.runner.identity.clone());

        loop {
            match self.runner.next_message().await? {
                Status::Message(TwitchCommands::Privmsg(msg)) => {
                    let args = Context::new(
                        msg.clone(),
                        responder.clone(),
                        self.state.clone(),
                        identity.clone(),
                        executor.clone(),
                    );
                    for active in actives {
                        let fut = active.call(args.clone());
                        executor
                            .spawn(async move {
                                if let Err(err) = fut.await {
                                    if !err.is::<DontCareSigil>() {
                                        log::error!("error: {}", err)
                                    }
                                }
                            })
                            .detach();
                    }
                }
                Status::Quit => break,
                Status::Eof => break,
                _ => continue,
            }
        }

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

    fn create_responder(mut writer: twitchchat::Writer, executor: &Executor) -> Responder {
        let (tx, mut rx) = async_channel::bounded::<Response>(32);

        executor
            .spawn(async move {
                while let Some(resp) = rx.next().await {
                    if let Err(..) = writer.encode(resp).await {
                        log::warn!("cannot write response");
                        break;
                    }
                }
                log::info!("end of respond loop");
            })
            .detach();

        Responder::new(tx)
    }
}
