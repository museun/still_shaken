#[macro_export]
macro_rules! really_dont_care {
    () => {
        return $crate::error::dont_care();
    };
}

#[derive(Debug)]
pub struct DontCareSigil;

impl std::fmt::Display for DontCareSigil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DontCareSigil")
    }
}

impl std::error::Error for DontCareSigil {}

pub trait DontCare<T> {
    fn dont_care(self) -> anyhow::Result<T>;
    fn is_real_error(self) -> Option<anyhow::Error>;
}

impl<T> DontCare<T> for Option<T> {
    fn dont_care(self) -> anyhow::Result<T> {
        self.ok_or_else(|| DontCareSigil.into())
    }
    fn is_real_error(self) -> Option<anyhow::Error> {
        None
    }
}

impl<T> DontCare<T> for Result<T, anyhow::Error> {
    fn dont_care(self) -> Self {
        self.map_err(|_| DontCareSigil.into())
    }

    fn is_real_error(self) -> Option<anyhow::Error> {
        is_real_error(self)
    }
}

pub fn dont_care<T>() -> anyhow::Result<T> {
    Err(DontCareSigil.into())
}

pub fn is_real_error<T>(err: anyhow::Result<T>) -> Option<anyhow::Error> {
    match err {
        Ok(..) => None,
        Err(err) if err.is::<DontCareSigil>() => None,
        Err(err) => Some(err),
    }
}
