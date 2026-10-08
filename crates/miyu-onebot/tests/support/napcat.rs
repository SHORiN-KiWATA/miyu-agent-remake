//! 假的 NapCat：照反向 WebSocket 的样子连进桥，发 OneBot v11 的事件，收桥调的动作、照样回。

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{WebSocketStream, client_async};

use super::{BOT, OWNER, TIME, TOKEN, within};

/// 令牌怎么出示。
#[derive(Debug, Clone, Copy)]
pub enum Auth<'a> {
    /// `Authorization: Bearer <令牌>`。
    Bearer(&'a str),
    /// `Authorization: Token <令牌>`。
    Token(&'a str),
    /// 查询参数 `access_token`。
    Query(&'a str),
    /// 不出示。
    Nothing,
}

/// 连上了的假 NapCat。
pub struct NapCat {
    ws: WebSocketStream<TcpStream>,
}

/// 连 `path`，令牌照 `auth` 出示，`X-Self-ID` 是 `self_id`（有的话）。被拒的交回 HTTP 状态码。
pub async fn napcat(
    port: u16,
    path: &str,
    auth: Auth<'_>,
    self_id: Option<i64>,
) -> Result<NapCat, u16> {
    let url = match auth {
        Auth::Query(token) => format!("ws://127.0.0.1:{port}{path}?access_token={token}"),
        _ => format!("ws://127.0.0.1:{port}{path}"),
    };
    let mut request = url.into_client_request().expect("合写法");
    let headers = request.headers_mut();
    match auth {
        Auth::Bearer(token) => {
            headers.insert(
                "Authorization",
                format!("Bearer {token}").parse().expect("合写法"),
            );
        }
        Auth::Token(token) => {
            headers.insert(
                "Authorization",
                format!("Token {token}").parse().expect("合写法"),
            );
        }
        Auth::Query(_) | Auth::Nothing => {}
    }
    if let Some(id) = self_id {
        headers.insert("X-Self-ID", id.to_string().parse().expect("合写法"));
    }
    headers.insert("X-Client-Role", "Universal".parse().expect("合写法"));
    let stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    match within("握手", client_async(request, stream)).await {
        Ok((ws, _)) => Ok(NapCat { ws }),
        Err(Error::Http(response)) => Err(response.status().as_u16()),
        Err(error) => panic!("握手出了别的错：{error}"),
    }
}

/// 照 `auth` 一直连，直到进得去（施工 O-20：令牌刚推来的，桥换上以前那一下还照旧的比）；回的不是
/// 401 的不再连。最多等十秒。
pub async fn admitted(port: u16, path: &str, auth: Auth<'_>, self_id: Option<i64>) -> NapCat {
    within("NapCat 进得去", async {
        loop {
            match napcat(port, path, auth, self_id).await {
                Ok(napcat) => return napcat,
                Err(401) => tokio::time::sleep(Duration::from_millis(100)).await,
                Err(status) => panic!("回的不是 401：{status}"),
            }
        }
    })
    .await
}

/// 一条私聊事件，照 NapCat 发的样子：`user` 发的、编号 `message_id`，时刻是 [`TIME`]。要改时刻、去掉时刻的，拿去改了再
/// [`NapCat::send`]。
pub fn private_frame(user: i64, message_id: i64, message: Value) -> Value {
    json!({
        "time": TIME,
        "self_id": BOT,
        "post_type": "message",
        "message_type": "private",
        "sub_type": "friend",
        "message_id": message_id,
        "user_id": user,
        "message": message,
        "raw_message": "",
        "font": 14,
        "sender": {"user_id": user, "nickname": "someone"},
    })
}

/// 照真机的样子连：`/onebot/v11/ws`，`Bearer` 令牌，带 `X-Self-ID`。
pub async fn owner_napcat(port: u16) -> NapCat {
    napcat(port, "/onebot/v11/ws", Auth::Bearer(TOKEN), Some(BOT))
        .await
        .expect("接了")
}

impl NapCat {
    /// 发一帧 JSON。
    pub async fn send(&mut self, frame: Value) {
        self.ws
            .send(Message::text(frame.to_string()))
            .await
            .expect("发得出");
    }

    /// 发一条私聊：`user` 发的、编号 `message_id`，`message` 是段的数组或者 CQ 字符串，时刻是 [`TIME`]。
    pub async fn private(&mut self, user: i64, message_id: i64, message: Value) {
        self.send(private_frame(user, message_id, message)).await;
    }

    /// 主人发一句文字。
    pub async fn owner_says(&mut self, message_id: i64, text: &str) {
        self.private(
            OWNER,
            message_id,
            json!([{"type": "text", "data": {"text": text}}]),
        )
        .await;
    }

    /// 下一个不是 `get_version_info` 的动作，回它成了；`get_version_info` 照 NapCat 的样子回。最多等十秒。
    pub async fn action(&mut self) -> Value {
        loop {
            let frame = within("桥调动作", self.ws.next())
                .await
                .expect("连接没断")
                .expect("读得到");
            let Message::Text(text) = frame else {
                continue;
            };
            let action: Value = serde_json::from_str(&text).expect("是 JSON");
            let data = match action["action"].as_str() {
                Some("get_version_info") => json!({
                    "app_name": "NapCat.Onebot",
                    "app_version": "4.8.0",
                    "protocol_version": "v11",
                }),
                _ => json!({"message_id": 1}),
            };
            self.send(json!({
                "status": "ok",
                "retcode": 0,
                "data": data,
                "message": "",
                "wording": "",
                "echo": action["echo"],
            }))
            .await;
            if action["action"] != "get_version_info" {
                return action;
            }
        }
    }

    /// 等桥连上就调的 `get_version_info`，照 NapCat 的样子回（施工 O-16：`/status` 照它说是哪个实现）。最多等十秒。
    pub async fn version(&mut self) {
        loop {
            let frame = within("桥问版本", self.ws.next())
                .await
                .expect("连接没断")
                .expect("读得到");
            let Message::Text(text) = frame else {
                continue;
            };
            let action: Value = serde_json::from_str(&text).expect("是 JSON");
            if action["action"] != "get_version_info" {
                continue;
            }
            self.send(json!({
                "status": "ok",
                "retcode": 0,
                "data": {"app_name": "NapCat.Onebot", "app_version": "4.8.0", "protocol_version": "v11"},
                "message": "",
                "wording": "",
                "echo": action["echo"],
            }))
            .await;
            return;
        }
    }

    /// 下一个动作是给主人的 `send_private_msg`：交回里面的字（只有一个文字段）。
    pub async fn reply(&mut self) -> String {
        let action = self.action().await;
        assert_eq!(action["action"], "send_private_msg", "{action}");
        assert_eq!(action["params"]["user_id"], OWNER, "{action}");
        let message = action["params"]["message"]
            .as_array()
            .expect("段的数组")
            .clone();
        assert_eq!(message.len(), 1, "{action}");
        assert_eq!(message[0]["type"], "text", "{action}");
        message[0]["data"]["text"]
            .as_str()
            .expect("有字")
            .to_string()
    }

    /// 关掉连接。
    pub async fn close(mut self) {
        if self.ws.close(None).await.is_err() {
            // 对面已经关了。
        }
    }
}
