//! `miyu ask --from` 不等了时给人看的字（施工 7-10，2026-09-30 主会话定，`docs/blueprint/cli/ask.md`「给人看的字」）：别的
//! harness 不打断她那一轮，只是不等了。

use super::Language;

impl Language {
    /// 写了 `--from`、她那一轮还在进行（还没认出来的也算）时不等了：`timed_out` 是到了 `--timeout`，不是的是按了 Ctrl+C。
    pub(crate) fn left_running(&self, timed_out: bool) -> String {
        match (self, timed_out) {
            (Language::Chinese, false) => "· 不等了，AI 那一轮还在接着跑".to_string(),
            (Language::Chinese, true) => "· 等到时间了，AI 那一轮还在接着跑".to_string(),
            (Language::English, false) => {
                "· Stopped waiting; the AI's turn keeps going".to_string()
            }
            (Language::English, true) => "· Time is up; the AI's turn keeps going".to_string(),
        }
    }
}
