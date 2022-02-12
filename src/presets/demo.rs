use crate::preset::Preset as PresetTrait;
use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

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

impl TryFrom<HashMap<String, String>> for Preset {
    type Error = anyhow::Error;
    fn try_from(form: HashMap<String, String>) -> anyhow::Result<Self> {
        match &**form.get(":duration").ok_or(anyhow!("no duration field"))? {
            "10" => Ok(Self::Sleep10),
            "60" => Ok(Self::Sleep60),
            _ => Err(anyhow!("invalid duration")),
        }
    }
}

impl PresetTrait for Preset {
    fn get_command(&self) -> String {
        String::from(match self {
            Self::Sleep10 => "sleep 10",
            Self::Sleep60 => "sleep 60",
        })
    }
    fn get_timeout(&self) -> u32 {
        match self {
            Self::Sleep10 => 15,
            Self::Sleep60 => 65,
        }
    }

    fn render_html() -> String {
        format!(
            r#"
<label for="duration">Sleep for:</label>
<select name=":duration" id="duration">
    <option value="10">10 seconds</option>
    <option value="60">60 seconds</option>
</select>        
"#
        )
    }
}
