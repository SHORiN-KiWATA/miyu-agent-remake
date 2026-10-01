//! 输入和动作的种类名，记进 `DEBUG` 的 `input`、`action` 两行（`28-运行日志.md` 第三节）：只写是哪
//! 一种，不写里面的字。

use miyu_kernel::event::TransientBody;
use miyu_kernel::session::{Action, Input};

/// 一条输入是哪一种。
pub(crate) fn input(input: &Input) -> &'static str {
    match input {
        Input::Command(_) => "command",
        Input::Stored { .. } => "stored",
        Input::Environment(_) => "environment",
        Input::Limits(_) => "limits",
        Input::TurnStartHooksDone { .. } => "turn_start_hooks_done",
        Input::RequestSent { .. } => "request_sent",
        Input::ModelDelta { .. } => "model_delta",
        Input::ModelEnded { .. } => "model_ended",
        Input::Woke { .. } => "woke",
        Input::ToolDone { .. } => "tool_done",
        Input::ToolProgress { .. } => "tool_progress",
        Input::ToolAsks { .. } => "tool_asks",
        Input::Restarting { .. } => "restarting",
        Input::Restored { .. } => "restored",
        Input::ReadBack { .. } => "read_back",
        Input::Reread { .. } => "reread",
        Input::Recalled { .. } => "recalled",
        Input::ToolGuarded { .. } => "tool_guarded",
        Input::JobEnded { .. } => "job_ended",
        Input::Watched { .. } => "watched",
        Input::WatchEnded { .. } => "watch_ended",
        Input::AsideSent { .. } => "aside_sent",
        Input::AsideDelta { .. } => "aside_delta",
        Input::AsideEnded { .. } => "aside_ended",
    }
}

/// 一个动作是哪一种。
pub(crate) fn action(action: &Action) -> &'static str {
    match action {
        Action::Append(_) => "append",
        Action::Reply { .. } => "reply",
        Action::Push(_) => "push",
        Action::RunTurnStartHooks { .. } => "run_turn_start_hooks",
        Action::CallModel { .. } => "call_model",
        Action::PushTransient(_) => "push_transient",
        Action::CancelModel { .. } => "cancel_model",
        Action::Wake { .. } => "wake",
        Action::RunTurnEndHooks { .. } => "run_turn_end_hooks",
        Action::StopTool { .. } => "stop_tool",
        Action::CancelTool { .. } => "cancel_tool",
        Action::GuardTool { .. } => "guard_tool",
        Action::AnswerTool { .. } => "answer_tool",
        Action::RunTool { .. } => "run_tool",
        Action::Restore { .. } => "restore",
        Action::Reread { .. } => "reread",
        Action::ReadBack { .. } => "read_back",
        Action::Recall { .. } => "recall",
        Action::Report(_) => "report",
        Action::StopJobs { .. } => "stop_jobs",
        Action::Aside { .. } => "aside",
    }
}

/// 一条输入、一个动作多不多：增量、执行中的输出一次回复有成百上千条，记在 `TRACE`，发行版里没有。
pub(crate) fn chatty_input(input: &Input) -> bool {
    matches!(
        input,
        Input::ModelDelta { .. } | Input::AsideDelta { .. } | Input::ToolProgress { .. }
    )
}

/// 同 [`chatty_input`]，看动作：推送增量、执行中的输出。
pub(crate) fn chatty_action(action: &Action) -> bool {
    matches!(
        action,
        Action::PushTransient(transient)
            if matches!(
                transient.body,
                TransientBody::ModelDelta(_) | TransientBody::ToolProgress(_)
            )
    )
}
