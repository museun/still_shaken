#![cfg_attr(debug_assertions, allow(dead_code, unused_variables))]
#[macro_use]
mod error;

mod bot;
pub use bot::Runner;

mod config;
pub use config::Config;

mod responder;

mod template;

mod format {
    pub trait FormatTime {
        fn timestamp(&self) -> String;
        fn relative_time(&self) -> String;
    }

    impl FormatTime for std::time::Duration {
        fn timestamp(&self) -> String {
            let seconds = self.as_secs();
            let (hours, minutes, seconds) =
                (seconds / (60 * 60), (seconds / 60) % 60, seconds % 60);
            if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            } else {
                format!("{:02}:{:02}", minutes, seconds)
            }
        }

        fn relative_time(&self) -> String {
            const TABLE: [(&str, u64); 4] = [
                ("days", 86400),
                ("hours", 3600),
                ("minutes", 60),
                ("seconds", 1),
            ];

            let mut secs = self.as_secs();
            let mut time = vec![];
            for (name, dur) in &TABLE {
                let div = secs / dur;
                if div > 0 {
                    time.push({
                        let s = if div > 1 {
                            name
                        } else {
                            &name[..name.len() - 1]
                        };
                        format!("{} {}", div, s)
                    });
                    secs -= dur * div;
                }
            }

            let len = time.len();
            if len > 1 {
                if len > 2 {
                    for t in &mut time.iter_mut().take(len - 2) {
                        t.push(',')
                    }
                }
                time.insert(len - 1, "and".into())
            }

            time.join(" ")

            // let dur = time::Duration::new(self.as_secs() as _, self.as_nanos() as _);

            // macro_rules! format_time {
            //     ($($expr:tt => $class:expr)*) => {{
            //         $(
            //             match dur.$expr() {
            //                 0 => { }
            //                 1 => { return format!("1 {}", $class) }
            //                 n => { return format!("{} {}s", n, $class) }
            //             };
            //         )*
            //         String::from("just now")
            //     }};
            // }

            // format_time! {
            //     whole_weeks   => "week"
            //     whole_days    => "day"
            //     whole_hours   => "hour"
            //     whole_minutes => "minute"
            //     whole_seconds => "second"
            // }
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
