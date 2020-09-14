#![cfg_attr(debug_assertions, allow(dead_code))]
use serde::{Deserialize, Serialize};

const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    // " (",
    // env!("CARGO_PKG_REPOSITORY"),
    // ")"
);

pub async fn get_json_with_body<T, E>(ep: &str, body: E) -> anyhow::Result<T>
where
    for<'de> T: Deserialize<'de> + Send + Sync + 'static,
    E: Serialize + Send + Sync + 'static,
{
    let ep = ep.to_string();
    blocking::unblock(move || sync_get_json_with_body(&*ep, &body)).await
}

pub async fn get_json<T>(ep: &str) -> anyhow::Result<T>
where
    for<'de> T: Deserialize<'de> + Send + Sync + 'static,
{
    let ep = ep.to_string();
    blocking::unblock(move || sync_get_json(&*ep)).await
}

pub fn sync_get_json_with_body<T, E>(ep: &str, body: &E) -> anyhow::Result<T>
where
    for<'de> T: Deserialize<'de> + Send + Sync + 'static,
    E: Serialize + Send + Sync + 'static + ?Sized,
{
    attohttpc::get(ep)
        .json(&body)?
        .header("User-Agent", USER_AGENT)
        .send()?
        .json()
        .map_err(Into::into)
}

pub fn sync_get_json<T>(ep: &str) -> anyhow::Result<T>
where
    for<'de> T: Deserialize<'de> + Send + Sync + 'static,
{
    attohttpc::get(ep)
        .header("User-Agent", USER_AGENT)
        .send()?
        .json()
        .map_err(Into::into)
}
