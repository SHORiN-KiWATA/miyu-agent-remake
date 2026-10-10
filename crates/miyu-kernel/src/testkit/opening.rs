//! 造替身（施工 7-5 从 `stage.rs` 挪出来）：人开的会话、`miyu ask` 开的一次性会话、别的会话派出来的子会话。

use std::collections::VecDeque;

use super::stage::Stage;
use crate::event::SessionCreated;
use crate::facts::Environment;
use crate::id::SessionId;
use crate::origin::By;
use crate::session::{Policy, Session};
use crate::time::Timestamp;

use super::stage::command_id;

/// 替身造的会话的编号：人开的、一次性的都是它（施工 1-13 再补：事实 `session` 写它）。
pub const SESSION: &str = "01a0f233-cfec-7023-8ed5-2a037a1d5ec8";

/// 替身造的子会话自己的编号，和父会话的不一样：子会话的事实 `session` 写它（施工 1-13 再补）。
pub const CHILD_SESSION: &str = "01a0f234-0a1b-7c2d-8e3f-4a5b6c7d8e9f";

/// 造会话时的样子：alice 的会话，在本机，工作区的权限，只读关着。
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 造的是哪一种会话。
enum Opening {
    /// 人开的会话。
    Main,
    /// `miyu ask` 开的一次性会话（施工 7-2）。
    Oneshot,
    /// 这个会话派出来的子会话（施工 7-5）。
    Child(SessionId),
}

impl Stage {
    /// 在 `now` 这一刻，照 `policy` 造一个会话，在 `environment` 里。造会话的命令编号是 `cmd-0`。
    ///
    /// # Panics
    ///
    /// 造会话那一条的写法坏了才会：那是替身自己的 bug。
    pub fn new(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
    ) -> Stage {
        Stage::created(policy, environment, now, Opening::Main)
    }

    /// 同上，造的是一次性的会话：`miyu ask` 开的那种（施工 7-2）。
    ///
    /// # Panics
    ///
    /// 同上。
    pub fn oneshot(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
    ) -> Stage {
        Stage::created(policy, environment, now, Opening::Oneshot)
    }

    /// 同上，造的是会话 `parent` 派出来的第 1 层子会话（施工 7-5）：`session.created` 带 `parent`、`depth`，由父会话造；
    /// 以后说的话都是父会话发的（交代、留言）。
    ///
    /// # Panics
    ///
    /// `parent` 不合会话编号的写法。
    pub fn child(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
        parent: &str,
    ) -> Stage {
        let parent =
            SessionId::parse(parent).unwrap_or_else(|e| panic!("父会话的编号写法坏了：{e}"));
        Stage::created(policy, environment, now, Opening::Child(parent))
    }

    fn created(
        policy: impl Fn() -> Policy + 'static,
        environment: Environment,
        now: Timestamp,
        opening: Opening,
    ) -> Stage {
        let mut by: By = serde_json::from_str(r#"{"kind":"person","account":"alice"}"#)
            .unwrap_or_else(|e| panic!("alice 的写法坏了：{e}"));
        let mut created: SessionCreated =
            serde_json::from_str(CREATED).unwrap_or_else(|e| panic!("造会话的写法坏了：{e}"));
        let mut id = SESSION;
        match opening {
            Opening::Main => {}
            Opening::Oneshot => created.oneshot = true,
            Opening::Child(parent) => {
                created.parent = Some(parent.clone());
                created.depth = Some(1);
                by = By::Session(crate::origin::Session { id: parent });
                id = CHILD_SESSION;
            }
        }
        let id = SessionId::parse(id).unwrap_or_else(|e| panic!("替身的会话编号写法坏了：{e}"));
        let (session, actions) = Session::create(
            id.clone(),
            command_id(0),
            by.clone(),
            now,
            created,
            policy(),
            environment.clone(),
        );
        let mut stage = Stage {
            session,
            id,
            policy: Box::new(policy),
            environment,
            log: Vec::new(),
            requests: Vec::new(),
            marks: Vec::new(),
            replies: Vec::new(),
            transients: Vec::new(),
            order: Vec::new(),
            ran: Vec::new(),
            lines: VecDeque::new(),
            plays: VecDeque::new(),
            verdicts: VecDeque::new(),
            injections: VecDeque::new(),
            presents: Vec::new(),
            held_model: None,
            hold_wakes: false,
            held_wake: None,
            held_tools: Vec::new(),
            stops: Vec::new(),
            now,
            next: 1,
            by,
            limits: None,
            summaries: None,
            disk: super::disk::Disk::default(),
            upward: Vec::new(),
            stopping: Vec::new(),
            recaps: Vec::new(),
            recap_lines: VecDeque::new(),
            held_recap: None,
            titles: Vec::new(),
            title_lines: VecDeque::new(),
            held_title: None,
            prepare: super::prepare::Prepares::default(),
            routing: super::routing::Routing::default(),
            sight: super::sight::Sight::default(),
            swap: None,
        };
        stage.settle(actions);
        stage
    }
}
