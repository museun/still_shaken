use std::{collections::HashMap, fmt::Display};

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    NestedTemplates,
    NonTerminated,
    EmptyTemplate,
    Custom(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Error {
    pub fn custom(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Custom(Box::new(err))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NestedTemplates => f.write_str("nested templates are not allowed"),
            Self::NonTerminated => f.write_str("non-terminated template found"),
            Self::EmptyTemplate => f.write_str("empty templates are not allowed"),
            Self::Custom(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Custom(err) => Some(&**err),
            _ => None,
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

pub trait DisplayFn: Send + Sync {
    fn display(&self) -> String;
}

macro_rules! display_for {
    ($($ty:ty)*) => {
        $(impl DisplayFn for $ty {
            fn display(&self) -> String {
                self.to_string()
            }
        })*
    };
}

display_for! {
    &String String &str str
    Box<str> std::sync::Arc<str>
    i8 i16 i32 i64 i128 isize
    u8 u16 u32 u64 u128 usize
    bool f32 f64
}

impl<T> DisplayFn for Option<T>
where
    T: DisplayFn,
{
    fn display(&self) -> String {
        self.as_ref().map(T::display).unwrap_or_default()
    }
}

impl<F, D> DisplayFn for F
where
    F: Fn() -> D + Send + Sync,
    D: Display,
{
    fn display(&self) -> String {
        (self)().to_string()
    }
}

#[derive(Default)]
pub struct Environment<'k, 'f> {
    pub env: HashMap<&'k str, &'f dyn DisplayFn>,
}

impl<'k, 'f> Environment<'k, 'f> {
    pub fn insert(mut self, key: &'k str, d: &'f dyn DisplayFn) -> Self {
        self.env.insert(key, d);
        self
    }

    fn resolve(&self, key: &str) -> Option<String> {
        self.env.get(key).map(|f| f.display())
    }
}

pub trait Template: Send + Sync {
    fn name(&self) -> &str;
    fn body(&self) -> &str;
    fn apply(&self, env: &Environment) -> Result<String>;
}

pub struct SimpleTemplate {
    pub name: String,
    pub data: String,
}

impl SimpleTemplate {
    pub fn new<N, I>(name: N, input: I) -> Self
    where
        N: Into<String>,
        I: Into<String>,
    {
        Self {
            name: name.into(),
            data: input.into(),
        }
    }
}

impl Template for SimpleTemplate {
    fn name(&self) -> &str {
        &self.name
    }

    fn body(&self) -> &str {
        &self.data
    }

    fn apply(&self, env: &Environment) -> Result<String> {
        let out = ParsedTemplate::parse(&self.data)?.apply(env);
        Ok(out)
    }
}

#[derive(Clone)]
pub struct ParsedTemplate<'a> {
    pub data: &'a str,
    pub keys: Vec<&'a str>,
}

impl<'a> ParsedTemplate<'a> {
    pub fn parse(input: &'a str) -> Result<Self> {
        Self::find_keys(input).map(|keys| Self { data: input, keys })
    }

    pub fn apply(&self, env: &Environment) -> String {
        let mut temp = self.data.to_string();
        for key in &self.keys {
            if let Some(val) = env.resolve(key) {
                let s = temp.replace(&format!("${{{}}}", key), &*val);
                let _ = std::mem::replace(&mut temp, s);
            }
        }
        temp.shrink_to_fit();
        temp
    }

    pub fn find_keys(input: &'a str) -> Result<Vec<&'a str>> {
        let (mut heads, mut tails) = (vec![], vec![]);

        let mut last = None;
        let mut iter = input.char_indices().peekable();
        while let Some((pos, ch)) = iter.next() {
            match (ch, iter.peek()) {
                ('$', Some((_, '{'))) => {
                    last.replace(pos);
                    heads.push(pos);
                    iter.next();
                }

                ('{', ..) if last.is_some() => return Err(Error::NestedTemplates),

                ('}', ..) if last.is_some() => {
                    tails.push(pos);
                    last.take();
                }
                _ => {}
            }
        }

        if heads.len() != tails.len() {
            return Err(Error::NonTerminated);
        }

        tails.reverse();

        let mut keys = Vec::with_capacity(heads.len());
        for head in heads {
            let tail = tails.pop().unwrap();
            if tail == head + 3 {
                return Err(Error::EmptyTemplate);
            }
            assert!(tail > head);
            keys.push(&input[head + 2..tail]);
        }
        Ok(keys)
    }
}
