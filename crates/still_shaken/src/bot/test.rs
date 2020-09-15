use crate::*;
use modules::Components;

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{Debug, Display},
    sync::Arc,
};

use async_mutex::Mutex;
use futures_lite::StreamExt;

use twitchchat::{
    messages::Privmsg, runner::Capabilities, runner::Identity, FromIrcMessage, IntoOwned,
};

pub struct TestRunner {
    state: State,
    msg: Privmsg<'static>,
    output: Vec<String>,
    executor: Executor,
    commands: Commands,
    passives: Passives,
}

impl TestRunner {
    pub fn new(data: impl Into<String>) -> Self {
        let executor = Executor::new(1);
        let passives = Passives::new(executor.clone());

        Self {
            msg: Self::build_msg(
                "@id=00000000-0000-0000-0000-000000000000",
                "test_user",
                "#test_channel",
                &data.into(),
            ),
            state: State::default(),
            output: Vec::new(),
            commands: Commands::default(),
            passives,
            executor,
        }
    }

    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.msg = Self::build_msg(
            self.msg.tags().raw_tags(),
            self.msg.name(),
            &channel.into(),
            self.msg.data(),
        );
        self
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.msg = Self::build_msg(
            self.msg.tags().raw_tags(),
            &user.into(),
            self.msg.channel(),
            self.msg.data(),
        );
        self
    }

    pub fn with_data(mut self, data: impl Into<String>) -> Self {
        self.msg = Self::build_msg(
            self.msg.tags().raw_tags(),
            self.msg.name(),
            self.msg.channel(),
            &data.into(),
        );
        self
    }

    pub fn with_broadcaster(self, user: impl Into<String>) -> Self {
        self.with_badge("broadcaster").with_user(user)
    }

    pub fn with_moderator(self, user: impl Into<String>) -> Self {
        self.with_badge("moderator").with_user(user)
    }

    pub fn with_vip(self, user: impl Into<String>) -> Self {
        self.with_badge("vip").with_user(user)
    }

    pub fn with_badge(mut self, badge: &str) -> Self {
        let tags = TagsBuilder::default()
            .merge_with(self.msg.tags().raw_tags())
            .add("badges", format!("{}/1", badge))
            .build();

        self.msg = Self::build_msg(&tags, self.msg.name(), self.msg.channel(), self.msg.data());
        self
    }

    pub fn reply(mut self, data: impl Display) -> Self {
        let tags = "@reply-parent-msg-id=00000000-0000-0000-0000-000000000000";
        let msg = format!(
            "{tags} PRIVMSG {channel} :{data}\r\n",
            channel = self.msg.channel(),
            data = data,
            tags = tags,
        );
        self.output.push(msg);
        self
    }

    pub fn say(mut self, data: impl Display) -> Self {
        let msg = format!(
            "PRIVMSG {channel} :{data}\r\n",
            channel = self.msg.channel(),
            data = data
        );
        self.output.push(msg);
        self
    }

    pub fn insert<T>(mut self, object: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        self.state.insert(object).unwrap();
        self
    }

    pub fn config(mut self, config: impl FnOnce(&mut Config)) -> Self {
        if !self.state.contains::<Config>() {
            self.state.insert(Config::default()).unwrap();
        }
        config(self.state.get_mut::<Config>().unwrap());
        self
    }

    pub fn with_module(mut self, ctor: impl Fn(&mut Components<'_>) -> anyhow::Result<()>) -> Self {
        if !self.state.contains::<Config>() {
            self.state.insert(Config::default()).unwrap();
        }

        let mut components = Components {
            config: self.state.get().unwrap(),
            commands: &mut self.commands,
            passives: &mut self.passives,
            executor: &self.executor,
        };

        ctor(&mut components).unwrap();

        self
    }

    pub fn run_commands(mut self, before: impl Fn()) {
        let commands = std::mem::take(&mut self.commands);
        before();
        let _ = self.run(commands);
    }

    pub fn run_passives(mut self, before: impl Fn()) {
        let passives = std::mem::replace(&mut self.passives, Passives::new(self.executor.clone()));
        before();
        let _ = self.run(passives);
    }

    pub fn run<H>(self, handler: H) -> H
    where
        H: Callable<Privmsg<'static>>,
    {
        let (tx, mut rx) = async_channel::unbounded();

        let responder = crate::responder::Responder::new(tx);
        let state = Arc::new(Mutex::new(self.state));

        let executor = Executor::new(1);
        let identity = Self::make_identity();

        let context = Context::new(self.msg, responder, state, identity, executor);

        let mut responses = self.output;
        responses.reverse();

        futures_lite::future::block_on(async move {
            match handler.call(context).await {
                Err(err) if err.is::<crate::error::DontCareSigil>() => {}
                Err(err) => panic!("{}", err),
                Ok(..) => {}
            }

            let len = responses.len();
            while let Some(msg) = rx.next().await {
                let msg = msg.to_string();
                let resp = match responses.pop() {
                    Some(resp) => resp,
                    None => panic!("a response was expected for:\n'{}'", msg.escape_debug()),
                };
                // TODO make this print better (it should parse both into Privmsg and then show the semantic difference)
                assert_eq!(resp, msg)
            }

            assert!(
                responses.is_empty(),
                "some responses remain:\n{pad:->40}\n{}\n{pad:->40}",
                responses
                    .iter()
                    .enumerate()
                    .fold(String::new(), |mut a, (i, c)| {
                        if !a.is_empty() {
                            a.push('\n');
                        }
                        a.push_str(&format!("#{}/{}: ", len - i, len));
                        a.push_str(&format!("{}: ", c.escape_debug()));
                        a
                    }),
                pad = ""
            );

            handler
        })
    }

    fn make_identity() -> Arc<Identity> {
        Arc::new(Identity::Full {
            name: "shaken_bot".into(),
            user_id: 241015868,
            display_name: Some("shaken_bot".into()),
            color: "#FF00FF".parse().unwrap(),
            caps: Capabilities {
                membership: false,
                commands: false,
                tags: true,
                unknown: <_>::default(),
            },
        })
    }

    fn build_msg(tags: &str, name: &str, channel: &str, data: &str) -> Privmsg<'static> {
        assert!(!name.is_empty());
        assert!(!channel.is_empty());
        assert!(!data.is_empty());

        let raw = if tags.is_empty() {
            format!(
                ":{name}!{name}@{name} PRIVMSG {channel} :{data}\r\n",
                name = name,
                channel = channel,
                data = data
            )
        } else {
            format!(
                "{tags} :{name}!{name}@{name} PRIVMSG {channel} :{data}\r\n",
                tags = tags,
                name = name,
                channel = channel,
                data = data
            )
        };

        let irc = twitchchat::irc::parse(&raw).next().unwrap().unwrap();
        Privmsg::from_irc(irc).unwrap().into_owned()
    }
}

#[derive(Default, Debug, Clone)]
pub struct TagsBuilder<'a> {
    map: HashMap<Cow<'a, str>, Cow<'a, str>>,
}

impl<'a> TagsBuilder<'a> {
    pub fn add(mut self, key: impl Into<Cow<'a, str>>, value: impl Into<Cow<'a, str>>) -> Self {
        self.map.insert(key.into(), value.into());
        self
    }

    pub fn merge_with(mut self, mut raw: &'a str) -> Self {
        raw = &raw[..raw.find(' ').unwrap()];
        while raw.starts_with('@') {
            raw = &raw[1..]
        }

        let iter = raw
            .split_terminator(';')
            .filter_map(|c| {
                let mut t = c.split('=');
                Some((t.next()?, t.next()?))
            })
            .map(|(k, v)| (Cow::from(k), Cow::from(v)));

        self.map.extend(iter);
        self
    }

    pub fn build(self) -> String {
        let mut cap = self
            .map
            .iter()
            .map(|(k, v)| (k.len() + v.len()))
            .sum::<usize>();

        // add the @ and all of the = and ;
        cap += 1 + self.map.len() * 2;

        self.map
            .into_iter()
            .fold(String::with_capacity(cap), |mut a, (k, v)| {
                if !a.is_empty() {
                    a.push(';');
                } else {
                    a.push('@');
                }

                a.push_str(&*k);
                a.push('=');
                a.push_str(&*v);
                a
            })
    }
}
