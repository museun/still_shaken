use super::{dont_care, Context, DontCare};

use futures_lite::StreamExt;
use twitchchat::messages::Privmsg;

pub async fn lookup_crate(mut context: Context) {
    while let Some(msg) = context.stream.next().await {
        let err = handle(&msg, &mut context).await;
        if let Some(err) = crate::error::is_real_error(err) {
            log::error!("{}", err);
        }
    }
}

async fn handle(msg: &Privmsg<'_>, context: &mut Context) -> anyhow::Result<()> {
    let mut iter = msg.data().splitn(2, char::is_whitespace);

    let input = match iter.next().dont_care()? {
        "!crate" | "!crates" | "!lookup" => iter.next().dont_care()?,
        _ => really_dont_care!(),
    };

    let mut crates = match lookup(input).await {
        Ok(crates) => crates,
        Err(err) => {
            let resp = "I cannot do a lookup on crates.io :(";
            return context.responder.reply(msg, resp);
        }
    };

    let c = match crates.pop() {
        Some(c) => c,
        None => {
            let resp = format!("I cannot find anything for '{}'", input);
            return context.responder.reply(msg, resp);
        }
    };

    let mut out = format!("{} = {}", c.name, c.max_version);
    if let Some(description) = c.description {
        fixup_description(&mut out, description);
    }
    context.responder.say(msg, out)?;

    if let Some(repo) = c.repository {
        let out = format!("repository: {}", repo);
        return context.responder.say(msg, out);
    }

    dont_care()
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
    out.push_str(crate::shrink_string(&*s, 400));
}

#[derive(serde::Deserialize)]
struct Crate {
    name: String,
    max_version: String,

    description: Option<String>,
    repository: Option<String>,
}

async fn lookup(query: &str) -> anyhow::Result<Vec<Crate>> {
    fn lookup(query: String) -> anyhow::Result<Vec<Crate>> {
        #[derive(serde::Deserialize)]
        struct Resp {
            crates: Vec<Crate>,
        }

        let req = attohttpc::get(&format!(
            "https://crates.io/api/v1/crates?page=1&per_page=1&q={}",
            query
        ));
        let resp = req.send()?.json::<Resp>()?;
        anyhow::Result::Ok(resp.crates)
    }

    let query = query.to_string();
    smol::unblock! { lookup(query )}
}
