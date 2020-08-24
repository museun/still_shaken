use still_shaken::{Config, Runner};

use rand::prelude::*;

fn init_logger() -> anyhow::Result<()> {
    alto_logger::init_alt_term_logger()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from(&[".env", ".env.dev"]);

    init_logger()?;

    let config = Config::load();

    let rng = rand::rngs::SmallRng::from_entropy();

    let fut = async move {
        let mut bot = Runner::connect(config).await?;
        bot.join_channels().await?;
        bot.run_to_completion(rng).await
    };
    smol::run(fut)
}
