#![cfg_attr(debug_assertions, allow(dead_code))]

pub async fn get_json_with_body<T, E>(ep: &str, body: E) -> anyhow::Result<T>
where
    for<'de> T: serde::Deserialize<'de> + Send + Sync + 'static,
    E: serde::Serialize + Send + Sync + 'static,
{
    let ep = ep.to_string();
    blocking::unblock(move || sync_get_json_with_body(&*ep, &body)).await
}

pub async fn get_json<T>(ep: &str) -> anyhow::Result<T>
where
    for<'de> T: serde::Deserialize<'de> + Send + Sync + 'static,
{
    let ep = ep.to_string();
    blocking::unblock(move || sync_get_json(&*ep)).await
}

pub fn sync_get_json_with_body<T, E>(ep: &str, body: &E) -> anyhow::Result<T>
where
    for<'de> T: serde::Deserialize<'de> + Send + Sync + 'static,
    E: serde::Serialize + Send + Sync + 'static + ?Sized,
{
    attohttpc::get(ep)
        .json(&body)?
        .send()?
        .json()
        .map_err(Into::into)
}

pub fn sync_get_json<T>(ep: &str) -> anyhow::Result<T>
where
    for<'de> T: serde::Deserialize<'de> + Send + Sync + 'static,
{
    attohttpc::get(ep).send()?.json().map_err(Into::into)
}
