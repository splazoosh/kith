//! Command handlers, one module per noun. Each handler takes the parsed args
//! (and, except `init`, an open `&Store`) and returns `anyhow::Result<()>`.

pub mod db;
#[cfg(feature = "dev")]
pub mod dev;
pub mod event;
pub mod export;
pub mod family;
pub mod import;
pub mod init;
pub mod media;
pub mod name;
pub mod person;
pub mod query;
pub mod search;
pub mod source;
