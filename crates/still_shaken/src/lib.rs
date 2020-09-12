#[macro_use]
mod error;

#[macro_use]
// pin-project-lite makes pub(crate) projections
#[allow(clippy::redundant_pub_crate)]
mod util;
pub use util::*;

mod bot;
pub use bot::*;

mod config;
pub use config::Config;

pub mod format;

mod http;

mod modules;
pub use modules::initialize_modules;

mod persist;

mod responder;
