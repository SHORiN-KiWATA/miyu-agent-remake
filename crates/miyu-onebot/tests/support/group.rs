//! 群里的假 NapCat（施工 O-22，`onebot.md` 第一条「群消息」「撤回」）：照 NapCat 的样子拼群消息、撤回的事件；连上的假 NapCat
//! 交给一个任务应答桥调的动作：`get_version_info` 照 NapCat 回，`get_group_member_info` 照给的群成员回（不在里面的回失败），
//! 别的（`send_group_msg`、`send_private_msg`）回成了、交出来给测试看。

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use super::{BOT, NapCat, TIME, within};

/// 一条群消息事件，照 NapCat 发的样子：群 `group` 里 `user` 发的、编号 `message_id`，群名片 `card`、昵称 `nickname`，时刻是
/// [`TIME`]。`message` 是段的数组或者 CQ 字符串。
pub fn group_frame(
    group: i64,
    user: i64,
    message_id: i64,
    message: Value,
    (card, nickname): (&str, &str),
) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "message",
        "message_type": "group",
        "sub_type": "normal",
        "message_id": message_id,
        "group_id": group,
        "user_id": user,
        "message": message,
        "raw_message": "",
        "font": 14,
        "sender": {"user_id": user, "nickname": nickname, "card": card, "role": "member"},
    })
}

/// 群里的撤回：群 `group` 里 `user` 发的第 `message_id` 条，`operator` 撤的。
pub fn group_recall(group: i64, user: i64, operator: i64, message_id: i64) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "notice",
        "notice_type": "group_recall",
        "group_id": group,
        "user_id": user,
        "operator_id": operator,
        "message_id": message_id,
    })
}

/// 私聊里的撤回：`user` 撤了自己发的第 `message_id` 条。
pub fn friend_recall(user: i64, message_id: i64) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "notice",
        "notice_type": "friend_recall",
        "user_id": user,
        "message_id": message_id,
    })
}

/// 一段文字。
pub fn plain(text: &str) -> Value {
    json!({"type": "text", "data": {"text": text}})
}

/// @ 一个号（`all` 是全体）。
pub fn at(qq: impl Into<Value>) -> Value {
    json!({"type": "at", "data": {"qq": qq.into()}})
}

/// 一个群成员：号、群名片、昵称。
pub type Member = (i64, &'static str, &'static str);

/// 交给任务应答的假 NapCat。放下了任务跟着停。
pub struct Answering {
    /// 要发给桥的帧。
    frames: mpsc::UnboundedSender<Value>,
    /// 桥调的、不是问版本和问群成员的动作。
    actions: mpsc::UnboundedReceiver<Value>,
    /// 桥问过哪些群成员（号），照先后。
    asked: Arc<Mutex<Vec<i64>>>,
    task: JoinHandle<()>,
}

impl NapCat {
    /// 交给一个任务应答：群成员照 `members` 回（`card`、`nickname`），不在里面的回失败。
    pub fn answering(self, members: &[Member]) -> Answering {
        let members = members.to_vec();
        let (frames, mut outgoing) = mpsc::unbounded_channel::<Value>();
        let (seen, actions) = mpsc::unbounded_channel();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let noted = Arc::clone(&asked);
        let (mut sink, mut stream) = self.ws.split();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    frame = outgoing.recv() => match frame {
                        Some(frame) => {
                            if sink.send(Message::text(frame.to_string())).await.is_err() {
                                return;
                            }
                        }
                        None => return,
                    },
                    read = stream.next() => {
                        let Some(Ok(Message::Text(text))) = read else {
                            match read {
                                Some(Ok(_)) => continue,
                                _ => return,
                            }
                        };
                        let action: Value = serde_json::from_str(&text).expect("是 JSON");
                        let answer = answer(&action, &members, &noted);
                        if action["action"] != "get_version_info"
                            && action["action"] != "get_group_member_info"
                            && seen.send(action).is_err()
                        {
                            return;
                        }
                        if sink.send(Message::text(answer.to_string())).await.is_err() {
                            return;
                        }
                    }
                }
            }
        });
        Answering {
            frames,
            actions,
            asked,
            task,
        }
    }
}

/// 照 NapCat 的样子回动作 `action`：问群成员的照 `members`，记下问的是谁。
fn answer(action: &Value, members: &[Member], asked: &Mutex<Vec<i64>>) -> Value {
    let (status, data) = match action["action"].as_str() {
        Some("get_version_info") => (
            "ok",
            json!({"app_name": "NapCat.Onebot", "app_version": "4.8.0", "protocol_version": "v11"}),
        ),
        Some("get_group_member_info") => {
            let user = action["params"]["user_id"].as_i64().expect("问的是号");
            asked.lock().expect("没 panic").push(user);
            match members.iter().find(|(id, _, _)| *id == user) {
                Some((id, card, nickname)) => (
                    "ok",
                    json!({"group_id": action["params"]["group_id"], "user_id": id, "card": card, "nickname": nickname}),
                ),
                None => ("failed", Value::Null),
            }
        }
        _ => ("ok", json!({"message_id": 1})),
    };
    json!({"status": status, "retcode": 0, "data": data, "message": "", "wording": "", "echo": action["echo"]})
}

impl Answering {
    /// 发一帧给桥。
    pub fn send(&self, frame: Value) {
        self.frames.send(frame).expect("任务还在");
    }

    /// 下一个桥调的动作（问版本、问群成员的不算），最多等十秒。
    pub async fn action(&mut self) -> Value {
        within("桥调动作", self.actions.recv())
            .await
            .expect("任务还在")
    }

    /// 下一个动作是发进群 `group` 的 `send_group_msg`：交回里面的字（只有一个文字段）。
    pub async fn group_reply(&mut self, group: i64) -> String {
        let action = self.action().await;
        assert_eq!(action["action"], "send_group_msg", "{action}");
        assert_eq!(action["params"]["group_id"], group, "{action}");
        let message = action["params"]["message"].as_array().expect("段的数组");
        assert_eq!(message.len(), 1, "{action}");
        message[0]["data"]["text"]
            .as_str()
            .expect("有字")
            .to_string()
    }

    /// 还没取的动作：没有的是空的。
    pub fn pending(&mut self) -> Option<Value> {
        self.actions.try_recv().ok()
    }

    /// 桥问过哪些群成员，照先后。
    pub fn asked(&self) -> Vec<i64> {
        self.asked.lock().expect("没 panic").clone()
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        self.task.abort();
    }
}
