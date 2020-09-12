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
    let ctrl_c = async_ctrlc::CtrlC::new()?;

    let mut bot = Runner::connect(config).await?;
    bot.join_channels().await?;

    let run = bot.run_to_completion(callables, executor);
    match ctrl_c.select(run).await {
        Left(..) => log::info!("got a ^C, exiting"),
        Right(result) => return result,
    };

    Ok(())
}
