use still_shaken::{Config, Runner};

use rand::prelude::*;

fn init_logger() -> anyhow::Result<()> {
    alto_logger::TermLogger::new(
        alto_logger::Options::default()
            .with_style(alto_logger::StyleConfig::MultiLine)
            .with_time(alto_logger::TimeConfig::unix_timestamp()),
    )?
    .init()
    .map_err(Into::into)
}

fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from(&[".env", ".env.dev"]);

    init_logger()?;

    let config = Config::load();

    let rng = rand::rngs::SmallRng::from_entropy();

    let fut = async move {
        let mut bot = Runner::connect(config).await?;
        bot.join_channel().await?;
        bot.run_to_completion(rng).await
    };
    smol::run(fut)
}
