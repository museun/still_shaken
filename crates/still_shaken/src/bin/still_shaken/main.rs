use still_shaken::{Config, Executor, FutExt as _, Left, Right, Runner};

fn init_logger() -> anyhow::Result<()> {
    alto_logger::init_alt_term_logger()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from(&[".env", ".env.dev"]);
    init_logger()?;

    let config = Config::load();

    let ctrl_c = async_ctrlc::CtrlC::new()?;

    let executor = Executor::new(2);
    let rng = fastrand::Rng::new();

    let fut = {
        let executor = executor.clone();
        async move {
            let mut bot = Runner::connect(config).await?;
            bot.join_channels().await?;

            match ctrl_c.select(bot.run_to_completion(rng, executor)).await {
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
