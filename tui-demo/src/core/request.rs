//! 一个命令写成核心的方法和参数（`docs/blueprint/protocol.md`「方法」）。

use serde_json::json;

use super::Command;

/// 一个命令写成核心的方法和参数；`/new` 不是发给会话的，交回 `None`。
pub(super) fn request(
    command: Command,
    session: &str,
    cwd: &str,
) -> Option<(&'static str, serde_json::Value)> {
    Some(match command {
        Command::Send { text, .. } => (
            "session.send",
            json!({"session": session, "text": text, "cwd": cwd}),
        ),
        Command::Interrupt { send } => {
            let queued = if send { "send" } else { "return" };
            (
                "session.interrupt",
                json!({"session": session, "queued": queued}),
            )
        }
        Command::Revert => ("session.revert", json!({"session": session})),
        Command::Unrevert => ("session.unrevert", json!({"session": session})),
        Command::Compact(words) => {
            let mut params = json!({"session": session});
            if let Some(words) = words {
                params["instructions"] = json!(words);
            }
            ("session.compact", params)
        }
        Command::Level(level) => {
            let mut params = level.permission();
            params["session"] = json!(session);
            ("session.set_permission_level", params)
        }
        Command::Clear => ("session.clear", json!({"session": session})),
        Command::Recap => ("session.recap", json!({"session": session})),
        Command::Configure(reference) => (
            "session.configure",
            json!({"session": session, "model": reference}),
        ),
        Command::Rename(title) => (
            "session.set_meta",
            json!({"session": session, "title": title}),
        ),
        Command::Stop(job) => ("job.stop", json!({"session": session, "job": job})),
        // 附件（换了的）在外面传好了再接上（`mod.rs` 的 `serve`）。
        Command::Redo { text, .. } => {
            let mut params = json!({"session": session});
            if let Some(text) = text {
                params["text"] = json!(text);
            }
            ("session.redo", params)
        }
        Command::New { .. }
        | Command::Watch(_)
        | Command::Unwatch(_)
        | Command::View(_)
        | Command::Open { .. }
        | Command::ListSessions
        | Command::Pin { .. }
        | Command::Delete(_)
        | Command::SetLanguage(_)
        | Command::FetchHuman(_)
        | Command::ListModels
        | Command::ListChoices
        | Command::Files { .. }
        | Command::ListEfforts
        | Command::SetChat(_)
        | Command::SetEffort { .. }
        | Command::Output { .. } => {
            return None;
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::request;
    use crate::core::Command;

    #[test]
    fn redo_only_says_what_changed() {
        // `/redo` 原样重来什么都不带；`/edit` 带改过的字（蓝图「斜杠命令」`/redo`、「输入框」第 13 条）。
        let plain = request(
            Command::Redo {
                text: None,
                files: None,
            },
            "s",
            "~",
        );
        assert_eq!(plain, Some(("session.redo", json!({"session": "s"}))));
        let edited = Command::Redo {
            text: Some("改过的".into()),
            files: None,
        };
        let expected = json!({"session": "s", "text": "改过的"});
        assert_eq!(request(edited, "s", "~"), Some(("session.redo", expected)));
    }

    #[test]
    fn a_rename_sets_the_title_and_a_bare_one_removes_it() {
        assert_eq!(
            request(Command::Rename(Some("回文".into())), "s", "~"),
            Some(("session.set_meta", json!({"session": "s", "title": "回文"})))
        );
        assert_eq!(
            request(Command::Rename(None), "s", "~"),
            Some(("session.set_meta", json!({"session": "s", "title": null})))
        );
    }

    #[test]
    fn a_recap_asks_for_this_session() {
        assert_eq!(
            request(Command::Recap, "s", "~"),
            Some(("session.recap", json!({"session": "s"})))
        );
    }
}
