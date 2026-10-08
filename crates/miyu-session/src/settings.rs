//! 会话这一块的配置项（`docs/blueprint/compaction.md` 第十五条第 6 条，施工 6-11 上）：提前压好 `compaction.prepare`。
//!
//! 会话 actor 在回合开始冻结这一轮的配置时读它（`actor/model.rs` 的 `turn_start`），跟着 `TurnStartHooksDone` 交给内核：
//! 下一轮生效。不开回合的手动压缩照上一轮的。

miyu_config::settings! {
    /// 压缩的配置。
    pub struct CompactionSettings in "compaction" {
        /// 提前压好：用量快到压缩线时在后台先把旧的那一段压好，到线直接换上，不用停下来等摘要。关了照到线再压。
        prepare: bool = true {
            kind: bool,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "advanced", group: "compaction", control: toggle },
        },
    }
}

#[cfg(test)]
mod tests;
