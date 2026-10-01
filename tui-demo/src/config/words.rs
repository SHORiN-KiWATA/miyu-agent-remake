//! 几块界面的字：回顾、改名、后台任务和别处来的话、换模型和冷却（`text/<语言>.json` 里各一格）。

use serde::Deserialize;

/// 回顾的字（蓝图「回顾」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecapTexts {
    /// 正文里那一段，`{text}` 是核心给的那一句。
    pub label: String,
    /// 发出去时弹的提示。
    pub working: String,
}

/// 改名的字（蓝图「改名」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameTexts {
    /// 改成了，`{title}` 是新标题。
    pub done: String,
    /// 去掉了标题。
    pub removed: String,
    /// 太长不发，`{max}` 是上限。
    pub too_long: String,
}

/// 后台命令、子代理、待办的字（蓝图「后台命令、子代理和侧边栏」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobTexts {
    /// 框下面那一行的后台按钮，`{count}` 在跑的命令数。
    pub button: String,
    /// 后台面板的标题。
    pub title: String,
    /// 面板标题下面那行，`{count}` 在跑的命令数。
    pub active: String,
    /// 面板最下面的按键提示。
    pub hint: String,
    /// 命令后面的状态：运行中 `{elapsed}`、完成 `{elapsed}`、失败 `{code}`、已停止。
    pub running: String,
    /// 见 `running`。
    pub done: String,
    /// 见 `running`。
    pub failed: String,
    /// 见 `running`。
    pub stopped: String,
    /// 正文里的通知：后台命令完成 `{title}` `{elapsed}`。
    pub done_note: String,
    /// 后台命令失败 `{title}` `{code}`。
    pub failed_note: String,
    /// 后台命令停了 `{title}`。
    pub stopped_note: String,
    /// 后台任务（子代理）完成 `{title}` `{elapsed}`。
    pub agent_note: String,
    /// 通知前面成功的记号（绿）。
    pub ok_mark: String,
    /// 通知前面失败的记号（红）。
    pub fail_mark: String,
    /// 通知前面停了的记号（暗）。
    pub stop_mark: String,
    /// 子代理状态行第一行：主会话。
    pub main: String,
    /// 收起来的，`{count}` 个。
    pub more: String,
    /// 命令被信号杀掉的状态：`{signal}`。
    pub killed: String,
    /// 撤销时停掉的状态。
    pub undone: String,
    /// 核心重启时停掉的状态。
    pub restarted: String,
    /// 展开一条命令、点开结束的那一行：它一个字都没输出。
    pub no_output: String,
    /// 后台面板里结束了的多于一个时收起来的那一行：`{count}` 收了几条。
    pub more_ended: String,
    /// 展开以后最后那一行：收起。
    pub less_ended: String,
    /// 命令被信号杀掉的通知：`{title}` `{signal}`。
    pub killed_note: String,
    /// 子代理被停掉的通知：`{title}`，后面接 `{why}`。
    pub agent_stopped_note: String,
    /// 通知后面接的为什么停：撤销时停的。
    pub why_undone: String,
    /// 通知后面接的为什么停：核心重启时停的。
    pub why_restarted: String,
    /// 子代理在想。
    pub doing_thinking: String,
    /// 子代理在说。
    pub doing_replying: String,
    /// 别处来的话那一行写的来处（蓝图「别处来的话」第 2 条）：同一个人在别处（网页、命令行）说的。
    pub from_person: String,
    /// 来处：主会话（在子会话里看，交代的活、留言）。
    pub from_main: String,
    /// 来处：别的主会话（核心 C-5），`{id}` 短编号、`{title}` 标题。
    pub from_session: String,
    /// 同上，没有标题的。
    pub from_session_untitled: String,
    /// 「空了告诉我」那一行里的会话：`{id}` 短编号、`{title}` 标题（核心 C-6）。
    pub peer_who: String,
    /// 同上，没有标题的。
    pub peer_who_untitled: String,
    /// 那个会话干完活空下来了，写成它回复了，`{who}`。
    pub peer_idle: String,
    /// 等了 12 小时作废了，`{who}`。
    pub peer_expired: String,
    /// 那个会话没了，`{who}`。
    pub peer_gone: String,
    /// 认不得的原因，`{who}`、`{reason}`。
    pub peer_other: String,
    /// 来处：这个会话派的子代理，`{job}` 任务编号。
    pub from_agent: String,
    /// 来处：别的 harness，`{name}` 它报的名字（洗过）。
    pub from_harness: String,
    /// 收着时那一行：`{from}` 来处，`{text}` 话的预览。
    pub received: String,
    /// 切进子会话时输入框上边框右边写的：`{title}` 子代理的名字。
    pub child_tag: String,
    /// 子代理报完了、还在看它时，状态行里写的「完成」（用时写在右边，不重复）。
    pub finished: String,
    /// 待办的进度，`{done}` `{total}`。
    pub todo: String,
    /// 待办长了：做完的收成一行，`{count}` 项。
    pub todo_folded: String,
    /// 待办长了：放不下的收成一行，`{count}` 项。
    pub todo_more: String,
}

/// 换模型、冷却的字（蓝图「配置与模型」第 2、7 条，核心 8-9）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelTexts {
    /// 出错换到别的端点，`{from}` → `{to}`（端点/模型）。
    pub failover: String,
    /// 都在冷却，还不知道几时恢复。
    pub cooling: String,
    /// 都在冷却，`{n}` 分钟后恢复。
    pub cooling_until: String,
    /// 钉着的模型没了、退回默认，`{from}` → `{to}`（引用）。
    pub replaced: String,
}
