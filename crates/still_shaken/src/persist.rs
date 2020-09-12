use std::path::Path;

use ::serde::{Deserialize, Serialize};
use anyhow::Context;

pub trait Persist<T>
where
    for<'de> T: Deserialize<'de> + Serialize,
{
    fn load(data: &[u8]) -> anyhow::Result<T>;

    fn load_from<P: AsRef<Path>>(file: P) -> anyhow::Result<T> {
        let data = std::fs::read(file)?;
        Self::load(&data)
    }

    fn save<P: AsRef<Path>>(path: P, element: &T) -> anyhow::Result<()>;
}

#[allow(dead_code)]
pub struct Json;

impl<T> Persist<T> for Json
where
    for<'de> T: Deserialize<'de> + Serialize,
{
    fn load(data: &[u8]) -> anyhow::Result<T> {
        serde_json::from_slice(data).with_context(|| "cannot deserialize via Persist")
    }

    fn save<P: AsRef<Path>>(path: P, element: &T) -> anyhow::Result<()> {
        let path = path.as_ref();
        std::fs::write(path, serde_json::to_vec_pretty(element)?)
            .with_context(|| anyhow::anyhow!("cannot save to '{}'", path.display()))
    }
}

pub struct Toml;

impl<T> Persist<T> for Toml
where
    for<'de> T: Deserialize<'de> + Serialize,
{
    fn load(data: &[u8]) -> anyhow::Result<T> {
        toml::from_slice(data).with_context(|| "cannot deserialize via Persist")
    }

    fn save<P: AsRef<Path>>(path: P, element: &T) -> anyhow::Result<()> {
        let path = path.as_ref();
        std::fs::write(path, toml::to_string_pretty(element)?.as_bytes())
            .with_context(|| anyhow::anyhow!("cannot save to '{}'", path.display()))
    }
}
