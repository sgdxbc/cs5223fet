use crate::preset::Preset as PresetTrait;
use serde_derive::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    Sleep10,
    Sleep60,
}

impl Display for Preset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sleep10 => write!(f, "sleep 10 seconds"),
            Self::Sleep60 => write!(f, "sleep 60 seconds"),
        }
    }
}

impl PresetTrait for Preset {
    type Iter = vec::IntoIter<Self>;
    fn enumerate() -> Self::Iter {
        vec![Self::Sleep10, Self::Sleep60].into_iter()
    }
}
