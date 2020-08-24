use rand::{prelude::*, rngs::SmallRng};
use still_shaken::{Config, Runner};

fn init_logger() -> anyhow::Result<()> {
    alto_logger::init_alt_term_logger()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from(&[".env", ".env.dev"]);
    init_logger()?;

    let rng = SmallRng::from_entropy();

    let config = Config::load();
    let fut = async move {
        let mut bot = Runner::connect(config).await?;
        bot.join_channels().await?;
        bot.run_to_completion(rng).await
    };
    smol::run(fut)
}
