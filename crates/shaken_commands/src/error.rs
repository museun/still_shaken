#[derive(Debug)]
pub enum Error {
    NoCommand,
    DuplicateKey(String),
    InvalidCharacters,
    RequiredInTail,
    OptionalAfterFlex,
    MultipleFlexible,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCommand => f.write_str("a command must be provided"),
            Self::DuplicateKey(key) => write!(f, "duplicate key found: {}", key),
            Self::InvalidCharacters => f.write_str("only alphanumeric keys are allowed"),
            Self::RequiredInTail => f.write_str("required cannot follow optional or flexible"),
            Self::OptionalAfterFlex => f.write_str("optional cannot follow flexible"),
            Self::MultipleFlexible => f.write_str("only a single flexible argument can exist"),
        }
    }
}

impl std::error::Error for Error {}
