use twitchchat::{messages::Privmsg, runner::Identity};

pub trait InspectErr<E> {
    fn inspect_err<F: Fn(&E)>(self, inspect: F) -> Self;
}

impl<T, E> InspectErr<E> for Result<T, E> {
    fn inspect_err<F: Fn(&E)>(self, inspect: F) -> Self {
        self.map_err(|err| {
            inspect(&err);
            err
        })
    }
}

pub fn shrink_string(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }

    let mut range = (1..max).rev();
    let max = loop {
        let i = match range.next() {
            Some(i) => i,
            None => break s.len(),
        };

        if s.is_char_boundary(i) {
            break i;
        }
    };

    &s[..max]
}

pub trait PrivmsgExt {
    fn is_mentioned(&self, identity: &Identity) -> bool;
    fn user_name(&self) -> &str;
}

impl<'a> PrivmsgExt for Privmsg<'a> {
    fn is_mentioned(&self, identity: &Identity) -> bool {
        let username = identity.username();
        match self.data().splitn(2, char::is_whitespace).next() {
            Some(s) if s.starts_with('@') && s.ends_with(username) => true,
            Some(s) if s.starts_with(username) && s.ends_with(':') => true,
            _ => false,
        }
    }
    fn user_name(&self) -> &str {
        self.display_name().unwrap_or_else(|| self.name())
    }
}
