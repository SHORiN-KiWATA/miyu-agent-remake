//! 会话这一块的配置项（`docs/blueprint/compaction.md` 第十五条第 6 条，施工 6-11 上）：提前压好 `compaction.prepare`。
//!
//! 会话 actor 在回合开始冻结这一轮的配置时读它（`actor/model.rs` 的 `turn_start`），跟着 `TurnStartHooksDone` 交给内核：
//! 下一轮生效。不开回合的手动压缩照上一轮的。
//!
//! 抽取的三项（施工 R-6 上，`memory.md` 第六条）：闲多久、攒几轮、照哪个模型。会话 actor 从忙到闲起闹钟时照这一轮的配置读。
//! 合并的两项（施工 R-7 上，第七条）：多久合一次、攒几个会话；抽取记下以后照那一轮的配置读。
//! 核心替人格记忆这个内置包声明（施工 F-4，设计 `30-插件框架.md` 第七节）：挂在「软件包」那一页、`memory` 那一组；包没装的照样
//! 认，设置页不画（`miyu-core` 的 `settings::Packaged`）。

use std::time::Duration;

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

miyu_config::settings! {
    /// 记忆抽取、合并的配置（施工 R-6 上、R-7 上）。
    pub struct MemorySettings in "memory" {
        /// 会话闲了多久才抽新的一段：短于常见的缓存寿命。
        extract_idle: Duration = "3m" {
            kind: duration [60, 3600],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", control: text },
        },
        /// 上次抽到以后至少有几轮她答了话才抽。
        extract_turns: i64 = 2 {
            kind: int [1, 100],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", control: number },
        },
        /// 整理记忆用的模型：模型或者 `@池`；没写的照 `models.chat`。
        organizer: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", control: text },
        },
        /// 离上次合并至少多久才再合（施工 R-7 上）。
        merge_every: Duration = "24h" {
            kind: duration [3600, 2592000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", control: text },
        },
        /// 上次合并以后至少抽过几个会话才再合（施工 R-7 上）。
        merge_sessions: i64 = 5 {
            kind: int [1, 100],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "memory", control: number },
        },
    }
}

#[cfg(test)]
mod tests;
