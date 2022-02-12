use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::iter::Iterator;

pub trait Preset
where
    Self: Send + Sync + Clone + Serialize + Deserialize<'static> + Display,
{
    type Iter: Iterator<Item = Self>;
    fn enumerate() -> Self::Iter;
}
