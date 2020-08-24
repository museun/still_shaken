use twitchchat::messages::Privmsg;

pub struct Cmd<'a> {
    pub head: &'a str,
    pub arg: Option<&'a str>,
    pub body: Option<&'a str>,
}

impl<'a> Cmd<'a> {
    pub fn parse(msg: &'a Privmsg<'_>) -> Option<Self> {
        const LEADER: &str = "!";

        if !msg.data().starts_with(LEADER) || msg.data().len() == LEADER.len() {
            return None;
        }

        let mut iter = msg
            .data()
            .get(LEADER.len()..)?
            .splitn(3, char::is_whitespace);

        let head = iter.next()?;
        let arg = iter.next().and_then(|c| match c {
            LEADER => None,
            c if c.starts_with(LEADER) => c.get(LEADER.len()..),
            c => Some(c),
        });
        let body = iter.next();

        Some(Self { head, arg, body })
    }
}
