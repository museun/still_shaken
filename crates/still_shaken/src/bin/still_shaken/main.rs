use still_shaken::*;

fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from(&[".env", ".env.dev"]);
    alto_logger::init_alt_term_logger()?;

    let config = Config::load();
    let executor = Executor::new(
        std::env::var("STILL_SHAKEN_THREADS")
            .ok()
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(1),
    );

    let mut commands = Commands::default();
    let mut passives = Passives::new(executor.clone());

    initialize_modules(
        &config, //
        &mut commands,
        &mut passives,
        &executor,
    )?;

    let callables: Vec<Box<ActiveCallable>> = vec![
        Box::new(commands), // actively called !commands
        Box::new(passives), // things that run on every Privmsg
    ];

    let fut = run_bot(config, executor.clone(), callables);
    futures_lite::future::block_on(executor.spawn(fut))
}

async fn run_bot(
    config: Config,
    executor: Executor,
    callables: Vec<Box<ActiveCallable>>,
) -> anyhow::Result<()> {
    let mut backoff = 0;
    loop {
        let ctrl_c = async_ctrlc::CtrlC::new()?;

        let mut bot = Runner::connect(config.clone()).await?;
        bot.join_channels().await?;

        let run = bot.run_to_completion(&callables, executor.clone());
        match ctrl_c.select(run).await {
            Left(..) => {
                log::info!("got a ^C, exiting");
                break Ok(());
            }
            Right(Err(err)) => {
                log::error!("error whilst running: {}", err);
                backoff += 5;
            }
            Right(Ok(..)) => backoff = 0,
        };

        log::info!("waiting {} seconds to reconnect", backoff);
        async_io::Timer::after(std::time::Duration::from_secs(backoff)).await;
    }
}
