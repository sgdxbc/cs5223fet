use crate::preset::Preset as PresetTrait;
use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    part: u32, // part 0 for all test, part 4 test 0 for all bonus test
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
        let (part, test) = from_str(form.get(":test").ok_or(anyhow!("no test field"))?)?;
        if part == 0 && test == 0 {
        } else if part == 1 && (1..=8).contains(&test) {
        } else if part == 2 && (1..=11).contains(&test) {
        } else if part == 3 && (1..=11).contains(&test) {
        } else if part == 4 && test == 0 {
        } else if part == 4 && (1..=26).contains(&test) {
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
        let run_test = part == 1
            || (part == 2 && (1..=7).contains(&test))
            || (part == 3 && (1..=7).contains(&test))
            || (part == 4 && ((1..=9).contains(&test) || (15..=21).contains(&test)));
        if part == 0 && log_level == LogLevel::Disable {
        } else if part == 4 && test == 0 && log_level == LogLevel::Disable {
        } else if run_test {
        } else if log_level == LogLevel::Disable {
        } else {
            return Err(anyhow!("cannot enable logging for specified test"));
        }
        let check = match &**form.get(":check").ok_or(anyhow!("no check field"))? {
            "yes" => true,
            "no" => false,
            _ => return Err(anyhow!("invalid check flag")),
        };
        if part == 0 {
        } else if part == 4 && test == 0 {
        } else if !run_test {
        } else if !check {
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
<p>Lab 4 Sharded Key/Value Service</p>
<select name=":test">
    <option value="{}">All tests</option>
    <option value="{}">All Bonus tests</option>
    {}
</select>
<label for="log-level">Log level:</label>
<select name=":log_level" id="log-level">
    {}
</select>
<label for="check">Check:</label>
<select name=":check" id="check">
    <option value="yes">Yes</option>
    <option value="no" selected="selected">No</option>
</select>
<ul>
    <li>If you want to enable logging, you must run one specific run test.</li>
    <li>If you want to enable checking, some of the running test must be search
    test.</li>
    <li>Enabling logging or checking will cause tests run differently compare to
    they do during grading. Do not enable them unless you have a good reason.
    </li>
</ul>
"#,
            to_string(&(0, 0)).unwrap(),
            to_string(&(4, 0)).unwrap(),
            (1..=8)
                .map(|i| (1, i))
                .chain((1..=11).map(|i| (2, i)))
                .chain((1..=11).map(|i| (3, i)))
                .chain((1..=26).map(|i| (4, i)))
                .map(|(part, test)| format!(
                    r#"<option value="{}"{}>Part {} Test {}</option>"#,
                    to_string(&(part, test)).unwrap(),
                    if part == 1 && test == 1 {
                        r#" selected="selected""#
                    } else {
                        ""
                    },
                    part,
                    test
                ))
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
            tar -xf submit.tar.gz && 
            find . -name "._*" | xargs -r rm &&
            ./run-tests.py --lab 4 {} {} {} {}"#,
            if self.part == 0 {
                format!("")
            } else {
                format!("--part {}", self.part)
            },
            if self.test == 0 {
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
        5 // extra credit for compile, collect output, etc.
        + if self.part == 0 {
            1265
        } else if self.part == 4 && self.test == 0 {
            1265
        } else {
            [
                [5, 5, 5, 5, 5, 5, 5, 5].as_slice(),
                [5, 20, 25, 25, 60, 40, 60, 90, 120, 120, 20].as_slice(),
                [5, 5, 10, 10, 60, 60, 60, 90, 120, 120, 20].as_slice(),
                [5, 20, 25, 25, 20, 60, 60, 40, 60, 90, 120, 120, 20, 20,
                5, 5, 10, 10, 60, 60, 60, 90, 120, 120, 20, 20].as_slice()
            ][self.part as usize][self.test as usize]
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
            } else if self.part == 4 && self.test == 0 {
                "All bonus tests"
            } else if self.part == 1 {
                match self.test {
                    1 => "TEST 1: Commands return OK (5pts)",
                    2 => "TEST 2: Initial query returns NO_CONFIG (5pts)",
                    3 => "TEST 3: Bad commands return ERROR (5pts)",
                    4 => "TEST 4: Initial config correct (5pts)",
                    5 => "TEST 5: Basic join/leave (5pts)",
                    6 => "TEST 6: Historical queries (5pts)",
                    7 => "TEST 7: Move command (5pts)",
                    8 => "TEST 8: Application deterministic (10pts)",
                    _ => unreachable!(),
                }
            } else if self.part == 2 {
                match self.test {
                    1 => "TEST 1: Single group, basic workload [RUN] (10pts)",
                    2 => "TEST 2: Multi-group join/leave [RUN] (15pts)",
                    3 => "TEST 3: Shards move when group joins [RUN] (15pts)",
                    4 => "TEST 4: Shards move when moved by ShardMaster [RUN] (15pts)",
                    5 => "TEST 5: Repeated shard movement [RUN] (20pts)",
                    6 => "TEST 6: Multi-group join/leave [RUN] [UNRELIABLE] (20pts)",
                    7 => "TEST 7: Repeated shard movement [RUN] [UNRELIABLE] (30pts)",
                    8 => "TEST 8: Single client, single group [SEARCH] (20pts)",
                    9 => "TEST 9: Single client, multi-group [SEARCH] (20pts)",
                    10 => "TEST 10: Multi-client, multi-group [SEARCH] (20pts)",
                    11 => "TEST 11: One server per group random search [SEARCH] (20pts)",
                    _ => unreachable!(),
                }
            } else if self.part == 3 {
                match self.test {
                    1 => "TEST 1: Single group, simple transactional workload [RUN] (5pts)",
                    2 => "TEST 2: Multi-group, simple transactional workload [RUN] (5pts)",
                    3 => "TEST 3: No progress when groups can't communicate [RUN] (10pts)",
                    4 => "TEST 4: Isolation between MultiPuts and MultiGets [RUN] (10pts)",
                    5 => "TEST 5: Repeated MultiPuts and MultiGets, different keys [RUN] (20pts)",
                    6 => "TEST 6: Repeated MultiPuts and MultiGets, different keys [RUN] [UNRELIABLE] (20pts)",
                    7 => "TEST 7: Repeated MultiPuts and MultiGets, different keys; constant movement [RUN] [UNRELIABLE] (20pts)",
                    8 => "TEST 8: Single client, single group; MultiPut, MultiGet [SEARCH] (20pts)",
                    9 => "TEST 9: Single client, multi-group; MultiPut, MultiGet [SEARCH] (20pts)",
                    10 => "TEST 10: Multi-client, multi-group; MultiPut, Swap, MultiGet [SEARCH] (20pts)",
                    11 => "TEST 11: One server per group random search [SEARCH] (20pts)",
                    _ => unreachable!()
                }
            } else if self.part == 4 {
                match self.test {
                    1 => "TEST 1: Single group, basic workload [RUN] (10pts)",
                    2 => "TEST 2: Multi-group join/leave [RUN] (15pts)",
                    3 => "TEST 3: Shards move when group joins [RUN] (15pts)",
                    4 => "TEST 4: Shards move when moved by ShardMaster [RUN] (15pts)",
                    5 => "TEST 5: Progress with majorities in each group [RUN] (15pts)",
                    6 => "TEST 6: Repeated partitioning of each group [RUN] (20pts)",
                    7 => "TEST 7: Repeated shard movement [RUN] (20pts)",
                    8 => "TEST 8: Multi-group join/leave [RUN] [UNRELIABLE] (20pts)",
                    9 => "TEST 9: Repeated shard movement [RUN] [UNRELIABLE] (30pts)",
                    10 => "TEST 10: Single client, single group [SEARCH] (20pts)",
                    11 => "TEST 11: Single client, multi-group [SEARCH] (20pts)",
                    12 => "TEST 12: Multi-client, multi-group [SEARCH] (20pts)",
                    13 => "TEST 13: One server per group random search [SEARCH] (20pts)",
                    14 => "TEST 14: Multiple servers per group random search [SEARCH] (20pts)",
                    15 => "TEST 15: Single group, simple transactional workload [RUN] (5pts)",
                    16 => "TEST 16: Multi-group, simple transactional workload [RUN] (5pts)",
                    17 => "TEST 17: No progress when groups can't communicate [RUN] (10pts)",
                    18 => "TEST 18: Isolation between MultiPuts and MultiGets [RUN] (10pts)",
                    19 => "TEST 19: Repeated MultiPuts and MultiGets, different keys [RUN] (20pts)",
                    20 => "TEST 20: Repeated MultiPuts and MultiGets, different keys [RUN] [UNRELIABLE] (20pts)",
                    21 => "TEST 21: Repeated MultiPuts and MultiGets, different keys; constant movement [RUN] [UNRELIABLE] (20pts)",
                    22 => "TEST 22: Single client, single group; MultiPut, MultiGet [SEARCH] (20pts)",
                    23 => "TEST 23: Single client, multi-group; MultiPut, MultiGet [SEARCH] (20pts)",
                    24 => "TEST 24: Multi-client, multi-group; MultiPut, Swap, MultiGet [SEARCH] (20pts)",
                    25 => "TEST 25: One server per group random search [SEARCH] (20pts)",
                    26 => "TEST 26: Multiple servers per group random search [SEARCH] (20pts)",
                    _ => unreachable!(),
                }
            } else {
                unreachable!()
            }
        )?;
        write!(f, ", log level: {:?}", self.log_level)?;
        write!(f, "{}", if self.check { ", check" } else { "" })
    }
}
