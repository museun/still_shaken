use modules::Components;

use crate::*;

pub struct Crates;
impl super::Initialize for Crates {
    fn initialize(Components { commands, .. }: &mut Components<'_>) -> anyhow::Result<()> {
        [
            "!crate <crate>",  // main command
            "!crates <crate>", // aliases
            "!lookup <crate>", // aliases
        ]
        .iter()
        .map(|cmd| commands.add(shaken_commands::Command::example(cmd).build()?, handle))
        .collect()
    }
}

async fn handle(ctx: Context<CommandArgs>) -> anyhow::Result<()> {
    let input = &ctx.args.map["crate"];

    let mut crates = match lookup(&*input).await {
        Ok(crates) => crates,
        Err(err) => {
            log::error!("cannot lookup crate: {}", err);
            let resp = "I cannot do a lookup on crates.io :(";
            return ctx.reply(resp);
        }
    };

    let c = match crates.pop() {
        Some(c) => c,
        None => {
            let resp = format!("I cannot find anything for '{}'", input);
            return ctx.reply(resp);
        }
    };

    let mut out = format!("{} = {}", c.name, c.max_version);
    if let Some(description) = c.description {
        fixup_description(&mut out, description);
    }
    ctx.say(out)?;

    if let Some(repo) = c.repository {
        let out = format!("repository: {}", repo);
        ctx.say(out)?;
    }

    ctx.say(format!(
        "documentation: https://docs.rs/{name}/{version}/{name}",
        name = c.name,
        version = c.max_version
    ))
}

fn fixup_description(out: &mut String, desc: String) {
    let s = desc.lines().fold(String::new(), |mut s, l| {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(l);
        s
    });

    out.push_str(" | ");
    out.push_str(crate::util::shrink_string(&*s, 400));
}

#[derive(serde::Deserialize)]
struct Crate {
    name: String,
    max_version: String,

    description: Option<String>,
    repository: Option<String>,
}

async fn lookup(query: &str) -> anyhow::Result<Vec<Crate>> {
    #[derive(serde::Deserialize)]
    struct Resp {
        crates: Vec<Crate>,
    }

    let ep = format!(
        "https://crates.io/api/v1/crates?page=1&per_page=1&q={}",
        query
    );

    crate::http::get_json(&ep)
        .await
        .map(|resp: Resp| resp.crates)
}
