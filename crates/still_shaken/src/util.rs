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
