//! 输入的清单（`docs/designs/02-内核.md` 第九节「不变量怎么查」）：会话的每一种输入一个名字。
//!
//! 从一条输入认出它是哪一种（[`InputKind::of`]），照输入和命令的每一种写，不用通配：以后加一种输入，
//! 这里编译不过，逼着把它加进清单；随机测试再查三百例里清单上的每一种都喂过，逼着把它加进生成器。

use super::*;

/// 名字和清单由这一张表生成，改不岔。
macro_rules! kinds {
    ($($(#[$doc:meta])* $kind:ident,)+) => {
        /// 输入的一种。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub(super) enum InputKind {
            $($(#[$doc])* $kind,)+
        }

        impl InputKind {
            /// 清单上的每一种，照表里的先后。
            pub(super) const ALL: &'static [InputKind] = &[$(InputKind::$kind,)+];
        }
    };
}

kinds! {
    /// 发一条消息。
    Send,
    /// 急着插话。
    Urgent,
    /// 打断，排着的接着发。
    Interrupt,
    /// 打断，排着的退回。
    TakeBack,
    /// 切权限级别。
    SetPermission,
    /// 回答确认。
    Decide,
    /// 回答一组题。
    Reply,
    /// 撤销。
    Revert,
    /// 恢复。
    Unrevert,
    /// 落盘了。
    Stored,
    /// 环境变了。
    Environment,
    /// 回合开始的挂接点跑完了。
    HooksDone,
    /// 请求发出去了。
    RequestSent,
    /// 模型的一段增量。
    ModelDelta,
    /// 模型说完了。
    ModelEnded,
    /// 工具执行完了。
    ToolDone,
    /// 工具执行中的输出。
    ToolProgress,
    /// 执行前的链判完了。
    ToolGuarded,
    /// 工具问人。
    ToolAsks,
    /// 要重启了。
    Restarting,
}

impl InputKind {
    /// 这条输入是哪一种。
    pub(super) fn of(input: &Input) -> InputKind {
        match input {
            Input::Command(received) => match &received.command {
                Command::Send { urgent: false, .. } => InputKind::Send,
                Command::Send { urgent: true, .. } => InputKind::Urgent,
                Command::Interrupt {
                    queued: Queued::Send,
                } => InputKind::Interrupt,
                Command::Interrupt {
                    queued: Queued::Return,
                } => InputKind::TakeBack,
                Command::SetPermission { .. } => InputKind::SetPermission,
                Command::Answer {
                    answer: Answer::Approval { .. },
                    ..
                } => InputKind::Decide,
                Command::Answer {
                    answer: Answer::Questions(_),
                    ..
                } => InputKind::Reply,
                Command::Revert { .. } => InputKind::Revert,
                Command::Unrevert => InputKind::Unrevert,
            },
            Input::Stored { .. } => InputKind::Stored,
            Input::Environment(_) => InputKind::Environment,
            Input::TurnStartHooksDone { .. } => InputKind::HooksDone,
            Input::RequestSent { .. } => InputKind::RequestSent,
            Input::ModelDelta { .. } => InputKind::ModelDelta,
            Input::ModelEnded { .. } => InputKind::ModelEnded,
            Input::ToolDone { .. } => InputKind::ToolDone,
            Input::ToolProgress { .. } => InputKind::ToolProgress,
            Input::ToolGuarded { .. } => InputKind::ToolGuarded,
            Input::ToolAsks { .. } => InputKind::ToolAsks,
            Input::Restarting { .. } => InputKind::Restarting,
        }
    }
}
