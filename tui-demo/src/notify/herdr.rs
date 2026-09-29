//! herdr（蓝图 `tui.md`「系统通知」第 6 条，照旧版 09-20 的集成）：在 herdr 的一个窗格里时，把在做、在等、空闲报给
//! herdr，由它在侧栏上显示、由它来响。不在 herdr 里这一段什么都不做。

use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use super::system::spawn;

/// 报给 herdr 的来源、名字：herdr 按来源记序号水位。
const SOURCE: &str = "custom:miyu";
const AGENT: &str = "miyu";

/// 报的状态（herdr 的 `--state`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 在做：一轮在跑。
    Working,
    /// 在等你：确认、提问的抽屉开着。
    Blocked,
    /// 空闲：等你打字。
    Idle,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Working => "working",
            State::Blocked => "blocked",
            State::Idle => "idle",
        }
    }
}

/// 坐在 herdr 的哪个窗格里、怎么叫它。
pub struct Herdr {
    binary: String,
    pane: String,
    /// 下一个序号：从进程启动那一刻的毫秒数起往上加。herdr 丢掉比水位小的，每次从 1 数的话第二次开界面的上报
    /// 全被丢掉（旧版 09-20 踩过）。
    next: u64,
    /// 上一次报的：一样的不再报。
    last: Option<State>,
}

impl Herdr {
    /// 照环境变量认：`HERDR_ENV` 是 `1`、有 `HERDR_PANE_ID` 和 `HERDR_BIN_PATH` 才算（用它注入的绝对路径，不受 PATH
    /// 影响）。
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Option<Self> {
        if var("HERDR_ENV").as_deref() != Some("1") {
            return None;
        }
        let pane = var("HERDR_PANE_ID").filter(|p| !p.is_empty())?;
        let binary = var("HERDR_BIN_PATH").filter(|b| !b.is_empty())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        Some(Self {
            binary,
            pane,
            next: now,
            last: None,
        })
    }

    /// 报一次状态，丢出去不等；和上一次一样的不报。
    pub fn report(&mut self, state: State) {
        if self.last == Some(state) {
            return;
        }
        self.last = Some(state);
        let args = self.report_args(state);
        spawn(&self.binary, &args);
    }

    fn report_args(&mut self, state: State) -> Vec<String> {
        let seq = self.seq();
        [
            "pane",
            "report-agent",
            &self.pane,
            "--source",
            SOURCE,
            "--agent",
            AGENT,
            "--state",
            state.word(),
            "--seq",
            &seq,
        ]
        .map(str::to_string)
        .to_vec()
    }

    /// 退还这个窗格：带序号（不带的 herdr 不认，侧栏一直挂着）。
    fn release_args(&mut self) -> Vec<String> {
        let seq = self.seq();
        [
            "pane",
            "release-agent",
            &self.pane,
            "--source",
            SOURCE,
            "--agent",
            AGENT,
            "--seq",
            &seq,
        ]
        .map(str::to_string)
        .to_vec()
    }

    fn seq(&mut self) -> String {
        let seq = self.next;
        self.next += 1;
        seq.to_string()
    }
}

/// 界面退出（崩了也算）：等着跑完退还，进程一没，丢出去的子进程也跟着没了。
impl Drop for Herdr {
    fn drop(&mut self) {
        let args = self.release_args();
        // 退还不了也只能这样：herdr 那边照它自己的超时收。
        let _released = Command::new(&self.binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Herdr, State};

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn outside_herdr_nothing_happens() {
        assert!(Herdr::detect(env(&[])).is_none());
        assert!(Herdr::detect(env(&[("HERDR_ENV", "1"), ("HERDR_PANE_ID", "p1")])).is_none());
        assert!(
            Herdr::detect(env(&[
                ("HERDR_PANE_ID", "p1"),
                ("HERDR_BIN_PATH", "/bin/herdr")
            ]))
            .is_none()
        );
    }

    #[test]
    fn reports_carry_a_rising_sequence_from_the_clock() {
        let mut h = Herdr::detect(env(&[
            ("HERDR_ENV", "1"),
            ("HERDR_PANE_ID", "p1"),
            ("HERDR_BIN_PATH", "/nonexistent/herdr"),
        ]))
        .unwrap();
        let first = h.report_args(State::Working);
        assert_eq!(
            first[..9],
            [
                "pane",
                "report-agent",
                "p1",
                "--source",
                "custom:miyu",
                "--agent",
                "miyu",
                "--state",
                "working"
            ]
        );
        let seq: u64 = first[10].parse().unwrap();
        assert!(seq > 1_700_000_000_000, "从毫秒数起：{seq}");
        let release = h.release_args();
        assert_eq!(release[1], "release-agent");
        assert_eq!(
            release[8].parse::<u64>().unwrap(),
            seq + 1,
            "退还也带序号，往上加"
        );
    }
}
