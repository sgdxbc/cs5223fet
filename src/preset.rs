use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Display;

pub trait Preset
where
    Self: Send
        + Sync
        + Clone
        + Serialize
        + for<'a> Deserialize<'a>
        + Display
        + TryFrom<HashMap<String, String>, Error = anyhow::Error>,
{
    fn render_html() -> String;
    fn get_command(&self) -> String;
    fn get_timeout(&self) -> u64;
}
