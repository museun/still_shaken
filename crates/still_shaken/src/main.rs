#![cfg_attr(debug_assertions, allow(dead_code,))]
#[macro_use]
mod error;

#[macro_use]
#[allow(clippy::redundant_pub_crate)] // pin-project-lite makes pub(crate) projections
mod util;

mod bot;
use bot::*;

mod modules;

mod config;
use config::Config;

mod responder;

mod format;
mod http;

use util::*;

fn init_logger() -> anyhow::Result<()> {
    alto_logger::init_alt_term_logger()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from(&[".env", ".env.dev"]);
    init_logger()?;

    let ctrl_c = async_ctrlc::CtrlC::new()?;

    let config = Config::load();
    let executor = Executor::new(6);

    let mut commands = Commands::default();
    let mut passives = Passives::new(executor.clone());

    modules::initialize_modules(&config, &mut commands, &mut passives, &executor)?;

    let actives: Vec<Box<ActiveCallable>> = vec![Box::new(commands), Box::new(passives)];

    let fut = {
        let executor = executor.clone();
        async move {
            let mut bot = Runner::connect(config).await?;
            bot.join_channels().await?;

            match ctrl_c
                .select(bot.run_to_completion(actives, executor))
                .await
            {
                Left(..) => {
                    log::info!("got a ^C, exiting");
                    Ok(())
                }
                Right(result) => result,
            }
        }
    };

    futures_lite::future::block_on(executor.spawn(fut))
}
