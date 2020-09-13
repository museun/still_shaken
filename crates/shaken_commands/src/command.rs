use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
};

use crate::{Arg, ArgKind, Error, ExtractResult};

#[derive(Default, Clone, Debug, Eq)]
pub struct Command {
    command: Box<str>,
    help: Box<str>,
    args: Box<[Arg]>,
}

impl Hash for Command {
    fn hash<H: Hasher>(&self, state: &mut H) {
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
    const START: &'static str = "<";
    const END: &'static str = ">";

    pub fn example(input: &str) -> Self {
        Self {
            help: input.into(),
            ..Self::default()
        }
    }

    pub fn build(self) -> Result<Self, Error> {
        self.parse()
    }

    pub const fn name(&self) -> &str {
        &*self.command
    }

    pub const fn help(&self) -> &str {
        &*self.help
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.args.iter().map(|s| &*s.data)
    }

    pub fn extract<'a, 'b>(&'a self, mut input: &'b str) -> ExtractResult<'a, 'b> {
        use ArgKind::*;

        // match and remove the command
        input = input.trim_start_matches(Self::LEADER);
        if !input.starts_with(&*self.command) {
            return ExtractResult::NoMatch;
        }
        // and any spaces between command and first argument
        input = input[self.command.len()..].trim_start();

        // if the input string is empty and we require an arg then this does not match
        if input.is_empty() && Self::contains(&self.args, &[Required]) {
            return ExtractResult::Required;
        }

        let mut map = HashMap::new();

        for Arg { data, ty } in &*self.args {
            match (ty, input.find(' ')) {
                // if we're at the end take the rest
                (Required, None) | (Optional, None) | (Flexible, ..) => {
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

    fn parse(mut self) -> Result<Self, Error> {
        use ArgKind::*;

        let mut iter = self
            .help
            .trim_start_matches(Self::LEADER)
            .split_terminator(' ');

        let command = iter.next().ok_or_else(|| Error::NoCommand)?;

        let mut seen = HashSet::new();
        let mut args = vec![];

        for key in iter.filter_map(Self::trim_args) {
            let (key, ty) = match () {
                _ if key.ends_with('?') => (key.trim_end_matches('?'), Optional),
                _ if key.ends_with("...") => (key.trim_end_matches("..."), Flexible),
                _ => (key, Required),
            };

            if !seen.insert(key) {
                return Err(Error::DuplicateKey(key.to_string()));
            }

            if !key.chars().all(char::is_alphanumeric) {
                return Err(Error::InvalidCharacters);
            }

            match ty {
                Required if Self::contains(&args, &[Optional, Flexible]) => {
                    return Err(Error::RequiredInTail)
                }

                Optional if Self::contains(&args, &[Flexible]) => {
                    return Err(Error::OptionalAfterFlex)
                }

                Flexible if Self::contains(&args, &[Flexible]) => {
                    return Err(Error::MultipleFlexible)
                }
                _ => {}
            }

            let data = key.into();
            args.push(Arg { data, ty })
        }

        self.command = command.into();
        self.args = args.into_boxed_slice();
        Ok(self)
    }

    fn trim_args(input: &str) -> Option<&str> {
        let next = input
            .trim_start_matches(Self::START)
            .trim_end_matches(Self::END);
        if next.len() != input.len() {
            Some(next)
        } else {
            None
        }
    }

    fn contains(args: &[Arg], kinds: &[ArgKind]) -> bool {
        args.iter().any(|c| kinds.contains(&c.ty))
    }
}
