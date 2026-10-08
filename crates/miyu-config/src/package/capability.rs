//! 扩展能力（施工 9-4 下上，`docs/designs/05-内核接口.md` 第三节）：清单的 `[process] capabilities` 声明要哪些，管理员
//! 在开的那一下批（`extensions.md`）。只认这张表里的十二个名字，先后照这张表。

/// 一种扩展能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// 向工具目录提供工具。
    Tools,
    /// 提供斜杠命令。
    Commands,
    /// 回合开始时往上下文注入内容。
    ContextInject,
    /// 工具执行前放行、拒绝，或要求询问人。
    ToolGuard,
    /// 工具执行后改写结果。
    ToolRewrite,
    /// 读取会话的原始事件。
    EventsRead,
    /// 追加自己命名空间下的事件。
    EventsWrite,
    /// 用命令驱动会话。
    SessionsDrive,
    /// 代表平台上的外部用户发命令：只给通讯平台的桥。
    ActForExternal,
    /// 访问网络。
    Network,
    /// 读指定的路径。
    FsRead,
    /// 写指定的路径。
    FsWrite,
}

impl Capability {
    /// 全部，照表的先后。
    pub const ALL: [Capability; 12] = [
        Capability::Tools,
        Capability::Commands,
        Capability::ContextInject,
        Capability::ToolGuard,
        Capability::ToolRewrite,
        Capability::EventsRead,
        Capability::EventsWrite,
        Capability::SessionsDrive,
        Capability::ActForExternal,
        Capability::Network,
        Capability::FsRead,
        Capability::FsWrite,
    ];

    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Tools => "tools",
            Capability::Commands => "commands",
            Capability::ContextInject => "context.inject",
            Capability::ToolGuard => "tool.guard",
            Capability::ToolRewrite => "tool.rewrite",
            Capability::EventsRead => "events.read",
            Capability::EventsWrite => "events.write",
            Capability::SessionsDrive => "sessions.drive",
            Capability::ActForExternal => "act_for_external",
            Capability::Network => "network",
            Capability::FsRead => "fs.read",
            Capability::FsWrite => "fs.write",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<Capability> {
        Capability::ALL
            .into_iter()
            .find(|capability| capability.as_str() == text)
    }
}
