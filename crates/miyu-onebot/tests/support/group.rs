//! 群里的假 NapCat（施工 O-22，`onebot.md` 第一条「群消息」「撤回」）：照 NapCat 的样子拼群消息、撤回的事件；连上的假 NapCat
//! 交给一个任务应答桥调的动作：`get_version_info` 照 NapCat 回，`get_group_member_info` 照给的群成员回（不在里面的回失败），
//! 别的（`send_group_msg`、`send_private_msg`、`delete_msg`）回成了、交出来给测试看，发消息的回的 `message_id` 照收到的先后
//! 从 [`FIRST_SENT`] 起一条加一（施工 O-23：她发过的编号要认得出「引用她」；撤回不占编号，施工 O-25 上）。真的 NapCat 并着办动作，回的先后不一定照发的先后：
//! [`NapCat::reversing`] 把头几条发消息的回应倒着回。
//!
//! 群的测试共用的（施工 O-23 从 `group.rs` 挪过来）：真核心照开关拉起真桥、系统的场所规则写好、假 NapCat 连上（[`started`]），
//! 读一个群的会话的事件（[`venue_events`]、[`until_event`]）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use miyu_http::testkit::Server;
use miyu_kernel::id::AccountId;
use miyu_session::Models;
use miyu_session::testkit::Script;
use miyu_store::log::read_events;
use miyu_store::root::DataRoot;

use super::ports::on_free_ports;
use super::spawning::{bridge_up, cli, ports_config_with, text};
use super::{BOT, Home, NapCat, TIME, owner_napcat, within};

/// 假 NapCat 回的第一个发出去的消息编号。
pub const FIRST_SENT: i64 = 90001;

/// [`NapCat::reversing`] 倒着回的两条回应之间隔多久。
const APART: Duration = Duration::from_millis(300);

/// 正向的等待最多多久：真的程序、真的核心，负载高时慢。
pub const WAIT: Duration = Duration::from_secs(60);

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
    /// 桥调的、不是问版本、问群成员、撤回的动作。
    actions: mpsc::UnboundedReceiver<Value>,
    /// 桥调的撤回（`delete_msg`，施工 O-25 上）：另放一处，群里的命令回执几秒后才撤，不插进别的测试等的动作里。
    recalls: mpsc::UnboundedReceiver<Value>,
    /// 桥问过哪些群成员（号），照先后。
    asked: Arc<Mutex<Vec<i64>>>,
    task: JoinHandle<()>,
}

impl NapCat {
    /// 交给一个任务应答：群成员照 `members` 回（`card`、`nickname`），不在里面的回失败。
    pub fn answering(self, members: &[Member]) -> Answering {
        self.reversing(members, 0)
    }

    /// 同 [`NapCat::answering`]，只是头 `held` 条发消息的回应先压着，攒够了倒着回，两条之间隔 [`APART`]：后发的那一条先回
    /// 到，先发的明明白白晚一截（施工 O-23）。
    pub fn reversing(self, members: &[Member], held: usize) -> Answering {
        let members = members.to_vec();
        let (frames, mut outgoing) = mpsc::unbounded_channel::<Value>();
        let (seen, actions) = mpsc::unbounded_channel();
        let (recalled, recalls) = mpsc::unbounded_channel();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let noted = Arc::clone(&asked);
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
                        let answer = answer(&action, &members, &noted, &mut sent);
                        let kind = action["action"].as_str().unwrap_or_default();
                        let recall = kind == "delete_msg";
                        let sending = !recall
                            && kind != "get_version_info"
                            && kind != "get_group_member_info";
                        let shown = if recall {
                            recalled.send(action)
                        } else if sending {
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
            asked,
            task,
        }
    }
}

/// 照 NapCat 的样子回动作 `action`：问群成员的照 `members`，记下问的是谁；发消息的回 `sent`，再加一。
fn answer(action: &Value, members: &[Member], asked: &Mutex<Vec<i64>>, sent: &mut i64) -> Value {
    let (status, data) = match action["action"].as_str() {
        Some("get_version_info") => (
            "ok",
            json!({"app_name": "NapCat.Onebot", "app_version": "4.8.0", "protocol_version": "v11"}),
        ),
        // 撤回（施工 O-25 上）：回成了，不占发出去的编号。
        Some("delete_msg") => ("ok", Value::Null),
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

/// 起一个照开关拉起桥的核心：系统的场所规则 `80-test.toml` 写成 `rules`，系统配置的 `[onebot]` 多写 `onebot`（自己人这类），
/// 桥起来、假 NapCat 连上交给任务应答，群成员照 `members`。挑的空端口被别人先占了的换一组再来（`ports.rs`）。交回核心、假
/// NapCat 和两个端口（NapCat 的、WebUI 的：重启以后等桥、再连）。
pub async fn started(
    script: &Script,
    rules: &str,
    onebot: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    started_with(script, rules, onebot, |napcat| napcat.answering(members)).await
}

/// 同 [`started`]，请求模型照 `models`（施工 O-25 上：她照台词说，[`super::speaking::Lines`]）。
pub async fn started_by(
    models: Arc<dyn Models>,
    rules: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    up(models, "", (rules, ""), "", |napcat| {
        napcat.answering(members)
    })
    .await
}

/// 同 [`started`]，假 NapCat 连上以后交给 `answer` 去应答（[`NapCat::reversing`] 这类）。
pub async fn started_with(
    script: &Script,
    rules: &str,
    onebot: &str,
    answer: impl FnOnce(NapCat) -> Answering,
) -> (Home, Answering, (u16, u16)) {
    up(Arc::new(script.clone()), "", (rules, ""), onebot, answer).await
}

/// 同 [`started`]，判官那一次（`model.call`）发到假服务器 `judge`（施工 O-23 下，[`super::judge`]）：她的回合照剧本 `script`。
/// `words` 不空的写成系统的违规词表（桥起来以前写好，不等重读）。
pub async fn started_judged(
    script: &Script,
    judge: &Server,
    (rules, words): (&str, &str),
    onebot: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    let models = super::judge::models(script);
    started_with_models(models, judge, (rules, words), onebot, members).await
}

/// 同 [`started_judged`]，请求模型照 `models`（[`super::judge::holding`] 这类）。
pub async fn started_with_models(
    models: Arc<dyn Models>,
    judge: &Server,
    (rules, words): (&str, &str),
    onebot: &str,
    members: &[Member],
) -> (Home, Answering, (u16, u16)) {
    let more = super::judge::config(judge);
    up(models, &more, (rules, words), onebot, |napcat| {
        napcat.answering(members)
    })
    .await
}

/// 起核心、拉起桥、连上假 NapCat：请求模型照 `models`，系统配置在端口、`[onebot]` 后面再接 `more`；系统的场所规则写成
/// `rules`，`words` 不空的写成系统的违规词表。
async fn up(
    models: Arc<dyn Models>,
    more: &str,
    (rules, words): (&str, &str),
    onebot: &str,
    answer: impl FnOnce(NapCat) -> Answering,
) -> (Home, Answering, (u16, u16)) {
    let (home, ports) = on_free_ports(async |listen, web| {
        let config = format!("{}{more}", ports_config_with(listen, web, onebot));
        let home = Home::spawning_with(Arc::clone(&models), &config);
        let dir = home.root.system().join("venues.d");
        std::fs::create_dir_all(&dir).expect("建得了目录");
        std::fs::write(dir.join("80-test.toml"), rules).expect("写得进");
        if !words.is_empty() {
            let modules = home.root.system().join("modules").join("onebot");
            std::fs::create_dir_all(&modules).expect("建得了目录");
            std::fs::write(modules.join("moderation.txt"), words).expect("写得进");
        }
        let started = cli(&home.root, &["start"]).await;
        assert_eq!(started.status.code(), Some(0), "{}", text(&started.stderr));
        bridge_up(&home.root, listen, web, None).await?;
        Ok((home, (listen, web)))
    })
    .await;
    let napcat = answer(owner_napcat(ports.0).await);
    (home, napcat, ports)
}

/// 停下桥、核心拉起的扩展。
pub async fn stopped(home: Home) {
    let stopped = cli(&home.root, &["stop"]).await;
    assert_eq!(stopped.status.code(), Some(0), "{}", text(&stopped.stderr));
    home.stop_extensions().await;
}

/// 系统账号 `onebot` 名下、场所是 `venue` 的那个会话：编号和事件（写成 JSON）；还没有的是空的。
pub fn venue_session(root: &DataRoot, venue: &str) -> Option<(String, Vec<Value>)> {
    let system = AccountId::parse("onebot").expect("合写法");
    root.sessions(&system)
        .unwrap_or_default()
        .iter()
        .map(|session| {
            let events: Vec<Value> = read_events(&root.session_dir(&system, session))
                .unwrap_or_default()
                .iter()
                .map(|event| serde_json::to_value(event).expect("写得成 JSON"))
                .collect();
            (session.as_str().to_string(), events)
        })
        .find(|(_, events)| {
            events
                .first()
                .is_some_and(|first| first["body"]["venue"] == venue)
        })
}

/// 场所是 `venue` 的那个会话的事件；还没有的是空的。
pub fn venue_events(root: &DataRoot, venue: &str) -> Vec<Value> {
    venue_session(root, venue)
        .map(|(_, events)| events)
        .unwrap_or_default()
}

/// 等到场所 `venue` 的会话里有合 `wanted` 的事件：交回那时的全部事件。
pub async fn until_event(
    root: &DataRoot,
    venue: &str,
    wanted: impl Fn(&Value) -> bool,
) -> Vec<Value> {
    until_events(root, venue, |events| events.iter().any(&wanted)).await
}

/// 等到场所 `venue` 的会话的事件合 `wanted`：交回那时的全部事件。
pub async fn until_events(
    root: &DataRoot,
    venue: &str,
    wanted: impl Fn(&[Value]) -> bool,
) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let events = venue_events(root, venue);
        if wanted(&events) {
            return events;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{venue} 等不到：{events:#?}\n运行日志：\n{}",
            run_log(root)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 事件里种类是 `kind` 的，照先后。
pub fn of_kind(events: &[Value], kind: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["kind"] == kind)
        .cloned()
        .collect()
}

/// 事件里人说的话（`message.user`），照先后。
pub fn said(events: &[Value]) -> Vec<Value> {
    of_kind(events, "message.user")
}

/// 一条人说的话的字。
pub fn words(said: &Value) -> &str {
    said["body"]["blocks"][0]["text"]
        .as_str()
        .unwrap_or_default()
}

/// 运行日志。
pub fn run_log(root: &DataRoot) -> String {
    std::fs::read_to_string(root.state().join("logs").join("onebot.log")).unwrap_or_default()
}
