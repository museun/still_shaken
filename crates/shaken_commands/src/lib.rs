use std::collections::HashMap;

#[derive(Debug)]
pub enum ExtractResult<'a, 'b> {
    Found(HashMap<&'a str, &'b str>),
    Required, // just print the help
    NoMatch,
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
enum ArgKind {
    Required,
    Optional,
    Flexible,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
struct Arg {
    data: Box<str>,
    ty: ArgKind,
}

mod command;
pub use command::Command;

mod error;
pub use error::Error;

#[cfg(test)]
mod tests;
