use std::collections::{HashMap, HashSet};

use anyhow::Context;
use twitchchat::messages::Privmsg;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
enum ArgType {
    Required,
    Optional,
    Flexible,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
struct Arg {
    data: Box<str>,
    kind: ArgType,
}

#[derive(Debug)]
pub enum ExtractResult<'a, 'b> {
    Found(HashMap<&'a str, &'b str>),
    Required, // just print the help
    NoMatch,
}

#[derive(Default, Clone, Debug, Eq)]
pub struct Command {
    elevated: bool,
    command: Box<str>,
    help: Box<str>,
    args: Box<[Arg]>,
}

impl std::hash::Hash for Command {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        state.write(self.command.as_bytes());
    }
}

impl PartialEq for Command {
    fn eq(&self, other: &Self) -> bool {
        self.help.eq(&other.help)
    }
}

impl Command {
    // TODO make this configurable
    pub const LEADER: &'static str = "!";

    pub fn example(input: &str) -> Self {
        let (elevated, command, args) = <_>::default();
        Self {
            help: input.into(),
            elevated,
            command,
            args,
        }
    }

    pub const fn elevated(mut self) -> Self {
        self.elevated = true;
        self
    }

    pub fn build(self) -> anyhow::Result<Self> {
        self.parse()
    }

    pub const fn name(&self) -> &str {
        &*self.command
    }

    pub const fn help(&self) -> &str {
        &*self.help
    }

    pub fn is_level_met(&self, msg: &Privmsg<'_>) -> bool {
        if !self.elevated {
            return true;
        }

        let badges = match msg.tags().get("badges") {
            Some(badges) => badges,
            None => return false,
        };

        use twitchchat::twitch::{Badge, BadgeKind::*};
        badges
            .split(',')
            .flat_map(Badge::parse)
            .fold(false, |ok, badge| {
                ok | matches!(badge.kind, Broadcaster | Moderator | VIP)
            })
    }

    pub fn extract<'a, 'b>(&'a self, mut input: &'b str) -> ExtractResult<'a, 'b> {
        // match and remove the command
        input = input.trim_start_matches(Self::LEADER);
        if !input.starts_with(&*self.command) {
            return ExtractResult::NoMatch;
        }
        // and any spaces between command and first argument
        input = input[self.command.len()..].trim_start();

        // if the input string is empty and we require an arg then this does not match
        if input.is_empty()
            && self
                .args
                .iter()
                .any(|Arg { kind, .. }| matches!(kind, ArgType::Required))
        {
            return ExtractResult::Required;
        }

        let mut map = HashMap::new();
        for Arg { data, kind } in &*self.args {
            match (kind, input.find(' ')) {
                // if we're at the end take the rest
                (ArgType::Required, None) | (ArgType::Optional, None) | (ArgType::Flexible, ..) => {
                    if !input.is_empty() {
                        map.insert(&**data, input);
                    }
                    break;
                }

                // otherwise take up to the space and continue
                (.., Some(next)) => {
                    map.insert(&**data, &input[..next]);
                    input = &input[next + 1..];
                }
            }
        }

        ExtractResult::Found(map)
    }

    fn parse(mut self) -> anyhow::Result<Self> {
        use ArgType::*;

        let mut iter = self
            .help
            .trim_start_matches(Self::LEADER)
            .split_terminator(' ');
        let command = iter.next().with_context(|| "a command must be provided")?;

        let mut seen = HashSet::<&str>::new();
        let mut args: Vec<Arg> = vec![];

        for part in iter {
            if part.starts_with('<') && part.ends_with('>') {
                let key = part.trim_start_matches('<').trim_end_matches('>');

                let (key, ty) = match () {
                    _ if key.ends_with('?') => (key.trim_end_matches('?'), Optional),
                    _ if key.ends_with("...") => (key.trim_end_matches("..."), Flexible),
                    _ => (key, Required),
                };
                anyhow::ensure!(seen.insert(key), "duplicate key found: {}", key);

                if !key.chars().all(char::is_alphanumeric) {
                    anyhow::bail!("only alphanumeric keys are allowed")
                }

                match ty {
                    Required => {
                        if args.iter().any(|c| matches!(c.kind, Optional | Flexible)) {
                            anyhow::bail!("required cannot follow optional or flexible")
                        }
                    }

                    Optional => {
                        if args.iter().any(|c| matches!(c.kind, Flexible)) {
                            anyhow::bail!("optional cannot follow flexible")
                        }
                    }

                    Flexible => {
                        if args.iter().any(|c| matches!(c.kind, Flexible)) {
                            anyhow::bail!("only a single flexible argument can exist")
                        }
                    }
                }

                args.push(Arg {
                    data: key.into(),
                    kind: ty,
                })
            }
        }

        self.command = command.into();
        self.args = args.into_boxed_slice();
        Ok(self)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn parse_command() {
//         let tests = vec![
//             "!foo <req> <opt?>",
//             "!foo <req> <opt?> <opt2?>",
//             "!foo <req> <opt?> <flex...>",
//             "!foo <req> <flex...>",
//             "!foo <flex...>",
//             "!foo <opt?> <flex...>",
//             "!foo <opt?> <opt2?> <flex...>",
//         ];

//         for test in tests {
//             // TODO assert
//             Command::example(test).build().unwrap();
//         }

//         let tests = vec![
//             "!foo <opt?> <req>",
//             "!foo <flex...> <opt?>",
//             "!foo <flex...> <req>",
//             "!foo <req> <opt?> <req2>",
//             "!foo <dup> <opt?> <dup>",
//             "!foo <flex1...> <flex2...>",
//             "!foo <opt?> <flex1...> <flex2...>",
//             "!foo <req> <opt?> <flex1...> <flex2...>",
//         ];

//         for test in tests {
//             // TODO assert
//             Command::example(test).build().unwrap_err();
//         }

//         let cmd = Command::example("!hello <name> <other?> <rest...>")
//             .build()
//             .unwrap();

//         assert!(!cmd
//             .extract("!hello world this is a test")
//             .unwrap()
//             .is_empty());
//         assert!(!cmd.extract("!hello world").unwrap().is_empty());

//         assert!(cmd.extract("!hello").is_none());
//         assert!(cmd.extract("!testing world this is a test").is_none());
//         assert!(cmd.extract("!").is_none());
//         assert!(cmd.extract("").is_none());

//         let cmd = Command::example("!hello <name> <other>").build().unwrap();
//         let map = cmd.extract("!hello world testing this").unwrap();
//         assert_eq!(map["name"], "world");
//         assert_eq!(map["other"], "testing");

//         let map = cmd.extract("!hello world testing").unwrap();
//         assert_eq!(map["name"], "world");
//         assert_eq!(map["other"], "testing");

//         let cmd = Command::example("!hello <name> <other> <tail...>")
//             .build()
//             .unwrap();
//         let map = cmd
//             .extract("!hello world testing this is the tail")
//             .unwrap();
//         assert_eq!(map["name"], "world");
//         assert_eq!(map["other"], "testing");
//         assert_eq!(map["tail"], "this is the tail");

//         let map = cmd.extract("!hello world testing").unwrap();
//         assert_eq!(map["name"], "world");
//         assert_eq!(map["other"], "testing");

//         // let tests = vec![
//         //     ("foo/1", false),
//         //     ("vip/1", true),
//         //     ("moderator/1", true),
//         //     ("broadcaster/1", true),
//         // ];

//         // let cmd = Command::example("!hello").unwrap().elevated();
//         // for (test, expected) in &tests {
//         //     assert_eq!(cmd.level_met(*test), *expected, "{}", test);
//         // }

//         // let cmd = Command::example("!hello").unwrap();
//         // for (test, _expected) in &tests {
//         //     assert!(cmd.level_met(*test))
//         // }
//     }
// }
