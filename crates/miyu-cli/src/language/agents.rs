//! `miyu ask` 等子代理时给人看的字（施工 7-9，样子 2026-09-30 项目主人定，`docs/blueprint/cli/ask.md`「给人看的字」）。

use super::Language;

impl Language {
    /// 等的那一行：还有 `count` 个子代理没报。
    pub(crate) fn waiting(&self, count: usize) -> String {
        match (self, count) {
            (Language::Chinese, _) => format!("· 等 {count} 个子代理回报…（按 Ctrl+C 不等了）"),
            (Language::English, 1) => {
                "· Waiting for 1 subagent to report… (Ctrl+C stops waiting)".to_string()
            }
            (Language::English, _) => {
                format!("· Waiting for {count} subagents to report… (Ctrl+C stops waiting)")
            }
        }
    }

    /// 子代理 `job`（标题 `title`）报回来了。
    pub(crate) fn reported(&self, job: &str, title: &str) -> String {
        match self {
            Language::Chinese => format!("· {job}「{title}」报回来了"),
            Language::English => format!("· {job} \u{201c}{title}\u{201d} reported back"),
        }
    }

    /// 等子代理的时候按了 Ctrl+C。
    pub(crate) fn stopped_waiting(&self) -> String {
        match self {
            Language::Chinese => {
                "· 不等了，子代理还在后台跑，下次 miyu ask -c 时她会看到结果".to_string()
            }
            Language::English => "· Stopped waiting; the subagents keep running, and she will see their results at the next miyu ask -c".to_string(),
        }
    }

    /// 到了 `--timeout`：`agents` 是还有子代理没报。
    pub(crate) fn timed_out(&self, agents: bool) -> String {
        match (self, agents) {
            (Language::Chinese, true) => "· 等到时间了，没回报的子代理还在后台跑".to_string(),
            (Language::Chinese, false) => "· 等到时间了".to_string(),
            (Language::English, true) => {
                "· Time is up; the subagents that have not reported keep running".to_string()
            }
            (Language::English, false) => "· Time is up".to_string(),
        }
    }
}
