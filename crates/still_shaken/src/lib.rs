#![cfg_attr(debug_assertions, allow(dead_code, unused_variables))]
mod bot;
pub use bot::Runner;

mod config;
pub use config::Config;

mod responder;

mod template;
