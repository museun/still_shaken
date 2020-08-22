use std::{collections::HashMap, fmt::Display};

pub trait DisplayFn: Send + Sync {
    fn display(&self) -> String;
}

macro_rules! so_lame {
    ($($ty:ty)*) => {
        $(impl DisplayFn for $ty {
            fn display(&self) -> String {
                self.to_string()
            }
        })*
    };
}

so_lame! {
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
        self.as_ref().map(|s| s.display()).unwrap_or_default()
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
pub struct Environment {
    pub env: HashMap<String, Box<dyn DisplayFn>>,
}

impl Environment {
    fn insert<K, F>(mut self, key: K, d: F) -> Self
    where
        K: Into<String>,
        F: DisplayFn + 'static,
    {
        self.env.insert(key.into(), Box::new(d));
        self
    }

    fn resolve(&self, key: &str) -> Option<String> {
        self.env.get(key).map(|f| f.display())
    }
}

pub trait Template: Send + Sync {
    fn name(&self) -> &str;
    fn body(&self) -> &str;
    fn apply(&self, env: &Environment) -> String;
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

    fn apply(&self, env: &Environment) -> String {
        let parsed = ParsedTemplate::parse(&self.data).unwrap();
        parsed.apply(env)
    }
}

#[derive(Clone)]
struct ParsedTemplate<'a> {
    data: &'a str,
    keys: Vec<&'a str>,
}

impl<'a> ParsedTemplate<'a> {
    fn parse(input: &'a str) -> anyhow::Result<Self> {
        Self::find_keys(input).map(|keys| Self { data: input, keys })
    }

    fn apply(&self, env: &Environment) -> String {
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

    fn find_keys(input: &'a str) -> anyhow::Result<Vec<&'a str>> {
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

                ('{', ..) if last.is_some() => {
                    anyhow::bail!("nested templates are not allowed");
                }

                ('}', ..) if last.is_some() => {
                    tails.push(pos);
                    last.take();
                }
                _ => {}
            }
        }

        if heads.len() != tails.len() {
            anyhow::bail!("non-terminated template found")
        }

        tails.reverse();

        let mut keys = Vec::with_capacity(heads.len());
        for head in heads {
            let tail = tails.pop().unwrap();
            if tail == head + 3 {
                anyhow::bail!("empty templates are not allowed")
            }
            assert!(tail > head);
            keys.push(&input[head + 2..tail]);
        }
        Ok(keys)
    }
}
