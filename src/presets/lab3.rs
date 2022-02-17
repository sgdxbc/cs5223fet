use crate::preset::Preset as PresetTrait;
use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    part: u32, // part 0 for all test
    test: u32,
    log_level: LogLevel,
    check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum LogLevel {
    Enable(String),
    Disable,
}

impl TryFrom<HashMap<String, String>> for Preset {
    type Error = anyhow::Error;
    fn try_from(form: HashMap<String, String>) -> anyhow::Result<Self> {
        let part = form
            .get(":part")
            .ok_or(anyhow!("no part field"))?
            .parse::<u32>()?;
        let test = form
            .get(":test")
            .ok_or(anyhow!("no test field"))?
            .parse::<u32>()?;
        if part == 0 && test == 0 {
        } else if part == 1 && (1..=27).contains(&test) {
        } else {
            return Err(anyhow!("invalid part and test combination"));
        }
        let log_level = match &**form
            .get(":log_level")
            .ok_or(anyhow!("no log level field"))?
        {
            level @ ("FINEST" | "FINER" | "FINE" | "INFO" | "WARNING" | "SEVERE") => {
                LogLevel::Enable(level.to_string())
            }
            "disable" => LogLevel::Disable,
            _ => return Err(anyhow!("invalid log level")),
        };
        if part == 0 && log_level == LogLevel::Disable {
        } else if part == 1 && (1..=19).contains(&test) {
        } else if part == 1 && (20..=27).contains(&test) && log_level == LogLevel::Disable {
        } else {
            return Err(anyhow!("cannot enable logging for specified test"));
        }
        let check = match &**form.get(":check").ok_or(anyhow!("no check field"))? {
            "yes" => true,
            "no" => false,
            _ => return Err(anyhow!("invalid check flag")),
        };
        if part == 0 {
        } else if part == 1 && (1..=19).contains(&test) && !check {
        } else if part == 1 && (20..=27).contains(&test) {
        } else {
            return Err(anyhow!("cannot check specified test"));
        }
        Ok(Self {
            part,
            test,
            log_level,
            check,
        })
    }
}

impl PresetTrait for Preset {
    fn render_html() -> String {
        format!(
            r#"
<p>Lab 3</p>
<select name=":part">
    <option value="0">All tests</option>
    <option value="1">Part 1</option>
</select>
<select name=":test">
    <option value="0">All tests</option>
    {}
</select>
<label for="log-level">Log level:</label>
<select name=":log_level" id="log-level">
    {}
</select>
<label for="check">Check:</label>
<select name=":check" id="check">
    <option value="yes">Yes</option>
    <option value="no">No</option>
</select>
"#,
            (1..=27)
                .map(|i| format!(r#"<option value="{0}">Test {0}</option>"#, i))
                .collect::<Vec<_>>()
                .join(""),
            ["disable", "FINEST", "FINER", "FINE", "INFO", "WARNING", "SEVERE"]
                .into_iter()
                .map(|level| format!(r#"<option value="{0}">{0}</option>"#, level))
                .collect::<Vec<_>>()
                .join(""),
        )
    }
    fn get_command(&self) -> String {
        format!(
            r#"
            cd $(mktemp -d);
            trap "rm -rf $(pwd)" EXIT TERM; 
            cp -r /usr/src/myapp/* .; 
            tar -xf submit.tar.gz && ./run-tests.py --lab 3 --part 1 {} {} {};"#,
            if self.part == 0 {
                format!("")
            } else {
                format!("--test {}", self.test)
            },
            match &self.log_level {
                LogLevel::Disable => format!(""),
                LogLevel::Enable(level) => format!("-g {}", level),
            },
            if self.check { "--checks" } else { "" }
        )
    }
    fn get_timeout(&self) -> u64 {
        10 // extra credit for compile, collect output, etc.
        + if self.part == 0 {
            700
        } else {
            match self.test {
                1 => 2,
                2 => 5,
                3 => 5,
                4 => 5,
                5 => 5,
                6 => 10,
                7 => 5,
                8 => 10,
                9 => 10,
                10 => 10,
                11 => 20,
                12 => 10,
                13 => 10,
                14 => 30,
                15 => 20,
                16 => 20,
                17 => 35,
                18 => 35,
                19 => 70,
                // search test time limit is subject to change
                // for now it is used time of my solution + 10s
                20 => 70,
                21 => 40,
                22 => 100,
                23 => 40,
                24 => 30,
                25 => 30,
                26 => 30,
                27 => 20, // actually it is 7s
                _ => unreachable!(),
            }
        }
    }
}

impl Display for Preset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            if self.part == 0 {
                "All tests"
            } else {
                match self.test {
                    1 => "TEST 1: Client throws InterruptedException [RUN]",
                    2 => "TEST 2: Single client, simple operations [RUN]",
                    3 => "TEST 3: Progress with no partition [RUN]",
                    4 => "TEST 4: Progress in majority [RUN]",
                    5 => "TEST 5: No progress in minority [RUN]",
                    6 => "TEST 6: Progress after partition healed [RUN]",
                    7 => "TEST 7: One server switches partitions [RUN]",
                    8 => "TEST 8: Multiple clients, synchronous put/get [RUN]",
                    9 => "TEST 9: Multiple clients, concurrent appends [RUN]",
                    10 => "TEST 10: Message count [RUN]",
                    11 => "TEST 11: Old commands garbage collected [RUN]",
                    12 => "TEST 12: Single client, simple operations [RUN] [UNRELIABLE]",
                    13 => "TEST 13: Two sequential clients [RUN] [UNRELIABLE]",
                    14 => "TEST 14: Multiple clients, synchronous put/get [RUN] [UNRELIABLE]",
                    15 => "TEST 15: Multiple clients, concurrent appends [RUN] [UNRELIABLE]",
                    16 => "TEST 16: Multiple clients, single partition and heal [RUN]",
                    17 => "TEST 17: Constant repartitioning, check maximum wait time [RUN]",
                    18 => "TEST 18: Constant repartitioning, check maximum wait time [RUN] [UNRELIABLE]",
                    19 => "TEST 19: Constant repartitioning, full throughput [RUN] [UNRELIABLE]",
                    20 => "TEST 20: Single client, simple operations [SEARCH]",
                    21 => "TEST 21: Single client, no progress in minority [SEARCH]",
                    22 => "TEST 22: Two clients, sequential appends visible [SEARCH]",
                    23 => "TEST 23: Two clients, five servers, multiple leader changes [SEARCH]",
                    24 => "TEST 24: Handling of logs with holes [SEARCH]",
                    25 => "TEST 25: Three server random search [SEARCH]",
                    26 => "TEST 26: Five server random search [SEARCH]",
                    27 => "TEST 27: Paxos runs in singleton group [RUN] [SEARCH]",
                    _ => unreachable!(),
                }
            }
        )?;
        write!(f, ", log level: {:?}", self.log_level)?;
        write!(f, "{}", if self.check { ", check" } else { "" })
    }
}
