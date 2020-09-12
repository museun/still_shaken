use std::{pin::Pin, task::Context, task::Poll};

use futures_lite::Future;
use twitchchat::{
    messages::Privmsg,
    runner::Identity,
    twitch::{Badge, BadgeKind},
};

#[macro_export]
macro_rules! into_iter {
    ($head:expr) => {
        std::iter::once($head)
    };

    ($head:expr, $($expr:expr),* $(,)?) => {
        into_iter!($head)$(.chain(into_iter!($expr)))*
    };
}

pub fn type_name<T>() -> &'static str {
    fn trim(input: &str) -> &str {
        let mut n = input.len();
        let left = input
            .chars()
            .take_while(|&c| {
                if c == '<' {
                    n -= 1;
                }
                !c.is_ascii_uppercase()
            })
            .count();
        &input[left..n]
    }

    let mut input = std::any::type_name::<T>();

    let original = input;
    loop {
        let start = input.len();
        input = trim(input);

        if input.contains('<') {
            input = trim(&input[1..])
        }

        match input.len() {
            0 => break original,
            d if d == start => break input,
            _ => {}
        }
    }
}

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
    fn is_above_user_level(&self) -> bool;
}

impl<'a> PrivmsgExt for Privmsg<'a> {
    fn is_mentioned(&self, identity: &Identity) -> bool {
        let username = identity.username();
        match self
            .data()
            .splitn(2, char::is_whitespace)
            .next()
            .map(|s| s.trim_end_matches(",.?!"))
        {
            Some(s) if s.starts_with('@') && s.ends_with(username) => true,
            Some(s) if s.starts_with(username) && s.ends_with(':') => true,
            _ => false,
        }
    }
    fn user_name(&self) -> &str {
        self.display_name().unwrap_or_else(|| self.name())
    }

    fn is_above_user_level(&self) -> bool {
        use BadgeKind::*;
        self.tags()
            .get("badges")
            .map(|badges| {
                badges
                    .split(',')
                    .flat_map(Badge::parse)
                    .fold(false, |ok, badge| {
                        ok | matches!(badge.kind, Broadcaster | Moderator | VIP)
                    })
            })
            .unwrap_or(false)
    }
}

pub trait FutExt: Sized {
    fn timeout(self, dur: std::time::Duration) -> TimeoutFut<Self>;
    fn select<Fut>(self, other: Fut) -> Select<Self, Fut>
    where
        Fut: Future;
    fn first<Fut>(self, other: Fut) -> Select<Self, Fut>
    where
        Fut: Future;
}

impl<F> FutExt for F
where
    F: Future,
{
    fn timeout(self, dur: std::time::Duration) -> TimeoutFut<Self> {
        TimeoutFut::new(self, dur)
    }

    fn select<Fut>(self, other: Fut) -> Select<Self, Fut> {
        Select::new(self, other, false)
    }

    fn first<Fut>(self, other: Fut) -> Select<Self, Fut> {
        Select::new(self, other, true)
    }
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

pub use Either::*;

pin_project_lite::pin_project! {
    pub struct Select<L, R> {
        #[pin] left: L,
        #[pin] right: R,

        biased: bool,
    }
}

impl<L, R> std::fmt::Debug for Select<L, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Select")
            .field("biased", &self.biased)
            .finish()
    }
}

impl<L, R> Select<L, R> {
    const fn new(left: L, right: R, biased: bool) -> Self {
        Self {
            left,
            right,
            biased,
        }
    }
}

impl<L, R> Future for Select<L, R>
where
    L: Future,
    R: Future,
{
    type Output = Either<L::Output, R::Output>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        macro_rules! poll {
            ($ident:ident => $expr:expr ) => {
                if let Poll::Ready(v) = this.$ident.poll(ctx) {
                    return Poll::Ready($expr(v));
                };
            };
        }

        if *this.biased || fastrand::bool() {
            poll!(left => Left);
            poll!(right => Right);
        } else {
            poll!(right => Right);
            poll!(left => Left);
        }

        Poll::Pending
    }
}

pin_project_lite::pin_project! {
    pub struct TimeoutFut<This> {
        #[pin] this: This,
        #[pin] after: async_io::Timer
    }
}

impl<This> TimeoutFut<This> {
    fn new(this: This, dur: std::time::Duration) -> Self {
        Self {
            this,
            after: async_io::Timer::after(dur),
        }
    }
}

impl<This> Future for TimeoutFut<This>
where
    This: Future,
{
    type Output = Result<This::Output, TimeoutErr>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.this.poll(ctx) {
            Poll::Ready(v) => Poll::Ready(Ok(v)),
            Poll::Pending => match this.after.poll(ctx) {
                Poll::Ready(..) => Poll::Ready(Err(TimeoutErr {})),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct TimeoutErr;

impl std::fmt::Display for TimeoutErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("future timed out")
    }
}

impl std::error::Error for TimeoutErr {}
