//! 群里的假 NapCat 交给一个任务应答（施工 O-22，O-25 中从 `group.rs` 挪出来）：桥调的动作照 NapCat 的样子回。
//! `get_version_info` 照 NapCat 回，`get_group_member_info` 照给的群成员回（不在里面的回失败），别的（`send_group_msg`、
//! `send_private_msg`、`delete_msg`）回成了、交出来给测试看，发消息的回的 `message_id` 照收到的先后从 [`FIRST_SENT`] 起一条
//! 加一（施工 O-23：她发过的编号要认得出「引用她」；撤回不占编号，施工 O-25 上）。真的 NapCat 并着办动作，回的先后不一定照
//! 发的先后：[`NapCat::reversing`] 把头几条发消息的回应倒着回。[`NapCat::refusing`] 发消息的一律回失败（施工 O-25 中）。贴、摘表情
//! （`set_msg_emoji_like`，施工 O-25 下）回成了、不占编号，另放一处给测试看（[`Answering::reacted`]）。平台工具（施工 O-31）：
//! 问群成员照给的身份回 `role`（[`NapCat::ranked`]，没给的是 `member`）；`set_group_ban`、`group_poke`、`friend_poke` 回成了、
//! 不占编号，交出来和发消息的放一处（[`Answering::action`]）；撤回编号是 [`UNRECALLABLE`] 的回失败（[`unrecallable`]）。取东西
//! （施工 O-33）：`get_msg` 照放进去的消息回（[`Answering::stock_message`]），`get_image`、`get_file` 照放进去的回应回
//! （[`Answering::stock_file`]），没放的回失败；不占编号，问过的记下（[`Answering::fetched`]）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use super::{NapCat, within};
/// 假 NapCat 回的第一个发出去的消息编号。
pub const FIRST_SENT: i64 = 90001;

/// [`NapCat::reversing`] 倒着回的两条回应之间隔多久。
const APART: Duration = Duration::from_millis(300);

/// [`NapCat::refusing`] 回失败时说的（施工 O-25 中）：前后带空白、240 个字符，桥记的 `detail` 去掉空白、截到 200 个字符。
pub fn refused() -> String {
    format!("  {}  ", "发送失败".repeat(60))
}

/// 一个群成员：号、群名片、昵称。
pub type Member = (i64, &'static str, &'static str);

/// 一个群成员在群里的身份（施工 O-31）：号、`role`（`owner`、`admin`、`member`）。
pub type Rank = (i64, &'static str);

/// 撤不了的那一条的编号（施工 O-31）：撤它回失败，说的是 [`unrecallable`]。
pub const UNRECALLABLE: i64 = 4040;

/// 撤 [`UNRECALLABLE`] 时 NapCat 说的：前后带空白、超过 200 个字符，桥交回的去掉空白、截到 200 个字符。
pub fn unrecallable() -> String {
    format!("  {}  ", "Recall failed".repeat(20))
}

/// 照 NapCat 的样子回、不占发出去的编号的平台动作（施工 O-31）。
const PLATFORM: [&str; 3] = ["set_group_ban", "group_poke", "friend_poke"];

/// 取东西的动作（施工 O-33）：照放进去的回，不占编号、不交给 [`Answering::action`]。
const FETCHING: [&str; 3] = ["get_msg", "get_image", "get_file"];

/// 放进去的消息、东西（施工 O-33）和问过的。
#[derive(Default)]
struct Stock {
    /// `get_msg` 的编号（字）→ 回应的 `data`。
    messages: HashMap<String, Value>,
    /// `get_image`、`get_file` 的 `file` → 回应的 `data`，或者回失败时说的。
    files: HashMap<String, Result<Value, String>>,
    /// 问过的：动作和 `message_id` 或 `file`，照先后。
    asked: Vec<(String, String)>,
}

/// 交给任务应答的假 NapCat。放下了任务跟着停。
pub struct Answering {
    /// 要发给桥的帧。
    frames: mpsc::UnboundedSender<Value>,
    /// 桥调的、不是问版本、问群成员、撤回的动作。
    actions: mpsc::UnboundedReceiver<Value>,
    /// 桥调的撤回（`delete_msg`，施工 O-25 上）：另放一处，群里的命令回执几秒后才撤，不插进别的测试等的动作里。
    recalls: mpsc::UnboundedReceiver<Value>,
    /// 桥调的贴、摘表情（`set_msg_emoji_like`，施工 O-25 下）：同撤回，另放一处。
    reactions: mpsc::UnboundedReceiver<Value>,
    /// 桥问过哪些群成员（号），照先后。
    asked: Arc<Mutex<Vec<i64>>>,
    /// 放进去的消息、东西和问过的（施工 O-33）。
    stock: Arc<Mutex<Stock>>,
    task: JoinHandle<()>,
}

impl NapCat {
    /// 交给一个任务应答：群成员照 `members` 回（`card`、`nickname`），不在里面的回失败。
    pub fn answering(self, members: &[Member]) -> Answering {
        self.reversing(members, 0)
    }

    /// 同 [`NapCat::answering`]，问群成员时照 `ranks` 回身份（施工 O-31）。
    pub fn ranked(self, members: &[Member], ranks: &[Rank]) -> Answering {
        self.serving(members, ranks, 0, false)
    }

    /// 同 [`NapCat::answering`]，只是头 `held` 条发消息的回应先压着，攒够了倒着回，两条之间隔 [`APART`]：后发的那一条先回
    /// 到，先发的明明白白晚一截（施工 O-23）。
    pub fn reversing(self, members: &[Member], held: usize) -> Answering {
        self.serving(members, &[], held, false)
    }

    /// 同 [`NapCat::answering`]，只是发消息的一律回失败（`status` 是 `failed`，`message` 是 [`refused`]，施工 O-25 中）。
    pub fn refusing(self, members: &[Member]) -> Answering {
        self.serving(members, &[], 0, true)
    }

    /// 交给一个任务应答：问群成员照 `ranks` 回身份，头 `held` 条发消息的回应倒着回（[`NapCat::reversing`]），`refuse` 的发消息
    /// 一律回失败。
    fn serving(self, members: &[Member], ranks: &[Rank], held: usize, refuse: bool) -> Answering {
        let members = members.to_vec();
        let ranks = ranks.to_vec();
        let (frames, mut outgoing) = mpsc::unbounded_channel::<Value>();
        let (seen, actions) = mpsc::unbounded_channel();
        let (recalled, recalls) = mpsc::unbounded_channel();
        let (reacted, reactions) = mpsc::unbounded_channel();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let noted = Arc::clone(&asked);
        let stock = Arc::new(Mutex::new(Stock::default()));
        let stocked = Arc::clone(&stock);
        let (mut sink, mut stream) = self.ws.split();
        let task = tokio::spawn(async move {
            let mut sent = FIRST_SENT;
            let mut holding = Vec::new();
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
                        let kind = action["action"].as_str().unwrap_or_default();
                        let mut answer = if FETCHING.contains(&kind) {
                            fetched(&action, &stocked)
                        } else {
                            answer(&action, (&members, &ranks), &noted, &mut sent)
                        };
                        let recall = kind == "delete_msg";
                        let reaction = kind == "set_msg_emoji_like";
                        let platform = PLATFORM.contains(&kind);
                        let sending = !recall
                            && !reaction
                            && !platform
                            && kind != "get_version_info"
                            && kind != "get_group_member_info"
                            && !FETCHING.contains(&kind);
                        if sending && refuse {
                            answer = json!({"status": "failed", "retcode": 1200, "data": null, "message": refused(), "wording": "", "echo": action["echo"]});
                        }
                        let shown = if recall {
                            recalled.send(action)
                        } else if reaction {
                            reacted.send(action)
                        } else if sending || platform {
                            seen.send(action)
                        } else {
                            Ok(())
                        };
                        if shown.is_err() {
                            return;
                        }
                        let mut answers = vec![answer];
                        if sending && holding.len() < held {
                            holding.append(&mut answers);
                            if holding.len() == held {
                                answers = holding.drain(..).rev().collect();
                            }
                        }
                        for (n, answer) in answers.into_iter().enumerate() {
                            if n > 0 {
                                tokio::time::sleep(APART).await;
                            }
                            if sink.send(Message::text(answer.to_string())).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        });
        Answering {
            frames,
            actions,
            recalls,
            reactions,
            asked,
            stock,
            task,
        }
    }
}

/// 照放进去的回取东西的动作 `action`（施工 O-33），记下问的是什么；没放的回失败，像 NapCat 说的那样。
fn fetched(action: &Value, stock: &Mutex<Stock>) -> Value {
    let kind = action["action"].as_str().unwrap_or_default().to_string();
    let params = &action["params"];
    let key = match kind.as_str() {
        "get_msg" => params["message_id"]
            .to_string()
            .trim_matches('"')
            .to_string(),
        _ => params["file"].as_str().unwrap_or_default().to_string(),
    };
    let mut stock = stock.lock().expect("没 panic");
    stock.asked.push((kind.clone(), key.clone()));
    let found = match kind.as_str() {
        "get_msg" => stock
            .messages
            .get(&key)
            .cloned()
            .ok_or_else(|| "消息不存在".to_string()),
        _ => stock
            .files
            .get(&key)
            .cloned()
            .unwrap_or_else(|| Err("file not found".to_string())),
    };
    match found {
        Ok(data) => {
            json!({"status": "ok", "retcode": 0, "data": data, "message": "", "wording": "", "echo": action["echo"]})
        }
        Err(said) => {
            json!({"status": "failed", "retcode": 1200, "data": null, "message": said, "wording": "", "echo": action["echo"]})
        }
    }
}

/// 照 NapCat 的样子回动作 `action`：问群成员的照 `members`、身份照 `ranks`，记下问的是谁；发消息的回 `sent`，再加一。
fn answer(
    action: &Value,
    (members, ranks): (&[Member], &[Rank]),
    asked: &Mutex<Vec<i64>>,
    sent: &mut i64,
) -> Value {
    if action["action"] == "delete_msg"
        && action["params"]["message_id"].to_string().trim_matches('"') == UNRECALLABLE.to_string()
    {
        return json!({"status": "failed", "retcode": 1200, "data": null, "message": unrecallable(), "wording": "", "echo": action["echo"]});
    }
    let (status, data) = match action["action"].as_str() {
        Some("get_version_info") => (
            "ok",
            json!({"app_name": "NapCat.Onebot", "app_version": "4.8.0", "protocol_version": "v11"}),
        ),
        // 撤回（施工 O-25 上）、贴摘表情（施工 O-25 下）、同意加好友（施工 O-27）：回成了，不占发出去的编号。
        Some(
            "delete_msg"
            | "set_msg_emoji_like"
            | "set_friend_add_request"
            | "set_group_ban"
            | "group_poke"
            | "friend_poke",
        ) => ("ok", Value::Null),
        Some("get_group_member_info") => {
            let user = action["params"]["user_id"].as_i64().expect("问的是号");
            asked.lock().expect("没 panic").push(user);
            let role = ranks
                .iter()
                .find(|(id, _)| *id == user)
                .map_or("member", |(_, role)| role);
            match members.iter().find(|(id, _, _)| *id == user) {
                Some((id, card, nickname)) => (
                    "ok",
                    json!({"group_id": action["params"]["group_id"], "user_id": id, "card": card, "nickname": nickname, "role": role}),
                ),
                None => ("failed", Value::Null),
            }
        }
        _ => {
            *sent += 1;
            ("ok", json!({"message_id": *sent - 1}))
        }
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

    /// 下一个动作是发进群 `group` 的 `send_group_msg`：交回段的数组（施工 O-25 上：看第一段带没带引用、@）。
    pub async fn group_message(&mut self, group: i64) -> Vec<Value> {
        let action = self.action().await;
        assert_eq!(action["action"], "send_group_msg", "{action}");
        assert_eq!(action["params"]["group_id"], group, "{action}");
        action["params"]["message"]
            .as_array()
            .expect("段的数组")
            .clone()
    }

    /// 下一个撤回（`delete_msg`）的参数，最多等十秒。
    pub async fn recalled(&mut self) -> Value {
        let action = within("桥撤回", self.recalls.recv())
            .await
            .expect("任务还在");
        action["params"].clone()
    }

    /// 还没取的撤回：没有的是空的。
    pub fn pending_recall(&mut self) -> Option<Value> {
        self.recalls.try_recv().ok()
    }

    /// 下一个贴、摘表情（`set_msg_emoji_like`）的参数，最多等十秒（施工 O-25 下）。
    pub async fn reacted(&mut self) -> Value {
        let action = within("桥贴摘表情", self.reactions.recv())
            .await
            .expect("任务还在");
        action["params"].clone()
    }

    /// 还没取的贴、摘表情：没有的是空的。
    pub fn pending_reaction(&mut self) -> Option<Value> {
        self.reactions.try_recv().ok()
    }

    /// 还没取的动作：没有的是空的。
    pub fn pending(&mut self) -> Option<Value> {
        self.actions.try_recv().ok()
    }

    /// 桥问过哪些群成员，照先后。
    pub fn asked(&self) -> Vec<i64> {
        self.asked.lock().expect("没 panic").clone()
    }

    /// 放进去一条消息（施工 O-33）：`get_msg` 问编号 `id` 的回 `data`（`message`、`group_id` 这些）。
    pub fn stock_message(&self, id: i64, data: Value) {
        let mut stock = self.stock.lock().expect("没 panic");
        stock.messages.insert(id.to_string(), data);
    }

    /// 放进去一样东西（施工 O-33）：`get_image`、`get_file` 问 `file` 是它的回 `data`；`Err` 的回失败，说的是它。
    pub fn stock_file(&self, file: &str, data: Result<Value, &str>) {
        let mut stock = self.stock.lock().expect("没 panic");
        stock
            .files
            .insert(file.to_string(), data.map_err(str::to_string));
    }

    /// 桥取过的：动作和 `message_id` 或 `file`，照先后（施工 O-33）。
    pub fn fetched(&self) -> Vec<(String, String)> {
        self.stock.lock().expect("没 panic").asked.clone()
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        self.task.abort();
    }
}
