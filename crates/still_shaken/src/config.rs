#[derive(Default, Clone, Debug, serde::Deserialize)]
pub struct Config {
    pub identity: Identity,
    pub modules: Modules,
}

#[derive(Default, Clone, Debug, serde::Deserialize)]
pub struct Identity {
    pub name: String,
    pub channels: Vec<String>,
}

#[derive(Default, Clone, Debug, serde::Deserialize)]
pub struct Modules {
    pub shaken: Shaken,
    pub commands: Commands,
}

#[derive(Default, Clone, Debug, serde::Deserialize)]
pub struct Shaken {
    pub host: String,
    pub timeout: u64,
    pub delay_lower: u64,
    pub delay_upper: u64,
    pub ignore_chance: f64,
}

#[derive(Default, Clone, Debug, serde::Deserialize)]
pub struct Commands {
    pub commands_file: String,
}

impl Config {
    pub fn load() -> Self {
        match Self::load_from_file() {
            Ok(config) => config,
            Err(..) => Self::write_default(),
        }
    }

    fn load_from_file() -> anyhow::Result<Self> {
        let data = std::fs::read_to_string("shaken.toml")?;
        toml::from_str(&data).map_err(Into::into)
    }

    fn write_default() -> ! {
        use std::path::PathBuf;
        eprintln!(
            "cannot load config. creating a default at '{}'",
            PathBuf::from("shaken.toml.example").to_string_lossy()
        );
        eprintln!(
            "copy it to '{}' and edit it then rerun",
            PathBuf::from("shaken.toml.example").to_string_lossy()
        );

        let data = toml::toml! {
            [identity]
            name     = "shaken_bot"
            channels = ["#museun", "#shaken_bot"]

            [modules.shaken]
            host          = "http://localhost:54612"
            timeout       = 1000
            delay_lower   = 100
            delay_upper   = 3000
            ignore_chance = 0.25

            [modules.commands]
            commands_file = "commands.toml"
        };
        let data = toml::to_string_pretty(&data).unwrap();
        std::fs::write("shaken.toml.example", &data).unwrap();

        std::process::exit(1);
    }
}
