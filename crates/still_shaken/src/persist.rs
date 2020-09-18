use ::serde::{Deserialize, Serialize};
use anyhow::Context;
use std::{marker::PhantomData, path::Path, path::PathBuf, time::SystemTime};

pub trait Persist<T>
where
    for<'de> T: Deserialize<'de> + Serialize,
{
    fn load(data: &[u8]) -> anyhow::Result<T>;

    fn load_from<P>(file: P) -> anyhow::Result<T>
    where
        P: AsRef<Path>,
    {
        Self::load(&std::fs::read(file)?)
    }

    fn save<P>(path: P, element: &T) -> anyhow::Result<()>
    where
        P: AsRef<Path>;
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

    fn save<P>(path: P, element: &T) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        std::fs::write(path, serde_json::to_vec_pretty(element)?)
            .with_context(|| format!("cannot save to '{}'", path.display()))
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

    fn save<P>(path: P, element: &T) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        std::fs::write(path, toml::to_string_pretty(element)?.as_bytes())
            .with_context(|| format!("cannot save to '{}'", path.display()))
    }
}

pub struct Cached<T, P = Json> {
    path: PathBuf,
    last: Option<SystemTime>,
    cached: Option<T>,
    _marker: PhantomData<P>,
}

impl<T, P> Cached<T, P>
where
    P: Persist<T>,
    for<'de> T: Serialize + Deserialize<'de>,
{
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            last: None,
            cached: None,
            _marker: PhantomData,
        }
    }

    pub fn with(path: impl Into<PathBuf>, item: T) -> anyhow::Result<Self> {
        let path = path.into();
        Self::get_modified_time(&path).map(|md| Self {
            path,
            last: Some(md),
            cached: Some(item),
            _marker: PhantomData,
        })
    }

    pub const fn path(&self) -> &Path {
        &self.path
    }

    pub fn ensure(&mut self) -> anyhow::Result<&T>
    where
        T: Default,
    {
        match std::fs::metadata(&self.path) {
            Ok(fi) if fi.is_file() => self.get(),
            _ => self.replace(T::default()),
        }
    }

    pub fn get(&mut self) -> anyhow::Result<&T> {
        let mt = Self::get_modified_time(&self.path)?;

        if self.cached.is_none()
            || self.last.is_none()
            || self.last.filter(|&last| last < mt).is_some()
        {
            let ok = P::load_from(&self.path)?;
            self.last.replace(mt);
            self.cached.replace(ok);
        }

        Ok(self.cached.as_ref().unwrap())
    }

    pub fn replace(&mut self, item: T) -> anyhow::Result<&T> {
        P::save(self.path, &item)?;

        self.cached.replace(item);
        self.last.replace(Self::get_modified_time(&self.path)?);

        Ok(self.cached.as_ref().unwrap())
    }

    pub fn backup(&self) -> anyhow::Result<PathBuf> {
        let item = self
            .cached
            .as_ref()
            .with_context(|| "no cached file loaded")?;

        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let path = &self.path;
        let path = path.with_file_name({
            path.file_stem()
                .and_then(|fs| fs.to_str())
                .map(|fi| {
                    PathBuf::from(format!("{}_backup_{}", fi, now))
                        .with_extension(path.extension().unwrap_or_default())
                })
                .with_context(|| "cannot create new name for backup file")?
        });

        P::save(&path, item).map(|_| path)
    }

    fn get_modified_time(path: &Path) -> anyhow::Result<SystemTime> {
        std::fs::metadata(path)
            .and_then(|md| md.modified())
            .with_context(|| format!("cannot get mtime for {}", path.display()))
    }
}
