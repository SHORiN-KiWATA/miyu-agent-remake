//! 等着回应、回应要另外办的请求：`serve.rs` 照请求编号记着，`asides.rs` 发不对着会话的请求时写上。

/// 等着回应、回应要另外办的请求（记着请求的编号）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Awaiting {
    /// 撤销：回应里给人看的几样交给界面。
    Revert,
    /// 恢复：同上。
    Unrevert,
    /// 说一句话：成了不用管，拒了要撤掉先画上的那句。
    Send,
    /// 重做：同上。
    Redo,
    /// 订阅另一个会话：回应里的限额交给界面，说明是哪个会话。
    Watch(String),
    /// 读一条后台命令的输出：哪个会话、任务编号。
    Output(String, String),
    /// 要回顾：交回的是上一句的，界面照回应画（写成了的照推送画）。
    Recap,
    /// 改名：成了弹一句（`None` 是去掉标题）。
    Rename(Option<String>),
    /// 带 `after` 订阅：回应到了算补完（「会话列表」第 5 条）。
    Replay(String),
    /// 列出会话：回应交给界面。
    List,
    /// 读界面语言：回应交给界面。
    UiLanguage,
    /// 要给人看的字：回应交给界面。
    Human,
    /// 要模型资料：冷却着的最早恢复交给界面。
    Models,
    /// 要 `/model` 框里的一行行。
    Choices,
    /// 要 `/effort` 框里的几级。
    Efforts,
    /// 要一个网址的卡片。
    LinkPreview(String),
    /// 一段段读一个 blob：哪个、读回来的。
    Blob(String, Vec<u8>),
    /// 画一张 mermaid 图：哪份源码。
    Mermaid(String),
    /// `@` 文件列表问的：哪个词。
    Files(crate::mention::Word),
    /// 换模型：成了交给界面（引用）。
    Configure(String),
    /// 配置页的一条请求：界面给的编号。
    Ask(u64),
}
