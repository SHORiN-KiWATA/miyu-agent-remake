//! 判官的替身（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 12、13 条）：测试里的核心请求模型分两头。她的回合照剧本
//! （[`Script`]，和别的桥的测试一样）；`model.call`（判官那一次，`purpose: "judge"`）走核心真的一次性入口，发到本机回环上的
//! 假服务器（`miyu_http::testkit::Server`，第几个连接回第几份）。照剧本回的端口没有一次性入口（`models.md`「怎么走」第十二
//! 条），所以判官要另走一头：核心那一头一个字没改，桥看到的就是真核心的 `model.call`。
//!
//! 系统配置多一家 `judge`（OpenAI 兼容、地址是假服务器、不带 key），`models.chat` 指 `judge/m`：判官不写模型就照它（[`config`]）。
//!
//! 要她这一轮还没说完时并进一条的（[`holding`]）：会话的端口头一次请求先压着，测试放行了才交给剧本。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Barrier, oneshot};

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Proxy, client};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::testkit::Script;
use miyu_session::{
    Cancel, ForSession, ModelData, ModelPort, Models, Observed, OneShot, Reports, Routes,
    TurnConfig,
};

/// 假服务器的那一家和模型：判官回答里的 `provider`、`model`。
pub const JUDGE_MODEL: &str = "judge/m";

/// 系统配置多的一段：一家 `judge` 在假服务器 `server` 上，`models.chat` 是它的 `m`。
pub fn config(server: &Server) -> String {
    format!(
        "\n[providers.judge]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"{JUDGE_MODEL}\"\n",
        server.base_url
    )
}

/// 请求模型的两头：会话的端口照剧本 `script`，一次性入口走真的路由（发到 [`config`] 写的那一家）。
pub fn models(script: &Script) -> Arc<dyn Models> {
    both(script, None)
}

/// 同 [`models`]，只是会话的头一次请求（她的第一轮）先压着，`release` 来了（或者发的一头放下了）才交给剧本。
pub fn holding(script: &Script, release: oneshot::Receiver<()>) -> Arc<dyn Models> {
    both(script, Some(release))
}

/// 两头：会话照剧本，头一次请求照 `hold` 压着；一次性入口走真的路由。
fn both(script: &Script, hold: Option<oneshot::Receiver<()>>) -> Arc<dyn Models> {
    let data = ModelData::new(
        Profiles::parse(
            &json!({"npm": {"@ai-sdk/openai-compatible": "openai-chat"}, "providers": {}}),
        )
        .expect("档案写法对"),
        Vendors::parse(&json!({})).expect("读得进"),
        None,
    );
    // 目录读没读成都算读完了：一次性入口先等它（`ModelData::wait`）。
    data.loaded(None, Observed::default());
    let routes = Routes {
        client: client(Proxy::Off).expect("造得出客户端"),
        direct: client(Proxy::Off).expect("造得出客户端"),
        data: Arc::new(data),
        idle: Duration::from_secs(60),
    };
    Arc::new(Both {
        script: script.clone(),
        routes,
        hold: Arc::new(Mutex::new(hold)),
    })
}

/// 会话照剧本、一次性入口照路由。
struct Both {
    script: Script,
    routes: Routes,
    /// 头一次请求等它：取走了就不再压。几个会话共用一份。
    hold: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
}

impl Models for Both {
    fn port(&self, session: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(Held {
            inner: self.script.port(session),
            hold: Arc::clone(&self.hold),
        })
    }

    fn one_shot(&self) -> Option<OneShot> {
        Some(OneShot::new(self.routes.clone()))
    }
}

/// 剧本的端口，头一次请求先压着。
struct Held {
    inner: Arc<dyn ModelPort>,
    hold: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
}

impl ModelPort for Held {
    fn model(&self) -> Model {
        self.inner.model()
    }

    fn reference(&self) -> Option<String> {
        self.inner.reference()
    }

    fn limits(&self) -> Limits {
        self.inner.limits()
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        let held = self.hold.lock().expect("没 panic").take();
        let Some(release) = held else {
            self.inner.call(seen, request, config, reports, cancel);
            return;
        };
        let (inner, config) = (Arc::clone(&self.inner), Arc::clone(config));
        tokio::spawn(async move {
            if release.await.is_err() {
                // 放行的一头放下了：照样交给剧本，不卡住。
            }
            inner.call(seen, request, &config, reports, cancel);
        });
    }
}

/// 起一个假服务器，照先后回 `replies`（一个连接一份）。
pub async fn judge(replies: Vec<Reply>) -> Server {
    Server::start(replies).await
}

/// 判官回的一段字 `text`：一段正文、说完、用量，照 OpenAI 兼容的流。
pub fn says(text: &str) -> Reply {
    Reply::stream(vec![Piece::Bytes(stream(text).into_bytes())])
}

/// 判官回的一段判断 `answer`（写成 JSON 的字）。
pub fn verdict(answer: Value) -> Reply {
    says(&answer.to_string())
}

/// 先等 `wait` 再回 `reply`：判官慢。
pub fn slow(wait: Duration, mut reply: Reply) -> Reply {
    reply.body.insert(0, Piece::Wait(wait));
    reply
}

/// 等 `gate` 放行（别的连接也走到这一道闸）再回 `reply`：几个请求都到了才一起回，不靠谁快。
pub fn gated(gate: &Arc<Barrier>, mut reply: Reply) -> Reply {
    reply.body.insert(0, Piece::Gate(Arc::clone(gate)));
    reply
}

/// 判官说回的一份：五维都是 9、该回、在跟她说话、没违规，理由 `reason`。
pub fn yes(reason: &str) -> Reply {
    verdict(json!({
        "relevance": 9, "willingness": 9, "social": 9, "timing": 9, "continuity": 9,
        "should_reply": true, "to_bot": true, "severity": 0, "reason": reason,
    }))
}

/// 判官说不回的一份：五维都是 1、不该回、不是在跟她说话、没违规，理由 `reason`。
pub fn no(reason: &str) -> Reply {
    verdict(json!({
        "relevance": 1, "willingness": 1, "social": 1, "timing": 1, "continuity": 1,
        "should_reply": false, "to_bot": false, "severity": 0, "reason": reason,
    }))
}

/// 假服务器收到的第 `n` 个请求的请求体。
pub fn asked(server: &Server, n: usize) -> Value {
    let received = server.received();
    let request = received
        .get(n)
        .unwrap_or_else(|| panic!("判官没收到第 {n} 个请求：一共 {}", received.len()));
    serde_json::from_slice(&request.body).expect("是 JSON")
}

/// 一份流：正文 `text`、说完、用量（输入 12，输出 3）。
fn stream(text: &str) -> String {
    let chunk = |choices: Value, usage: Value| {
        let event = json!({"id": "c1", "object": "chat.completion.chunk", "model": "m",
            "choices": choices, "usage": usage});
        format!("data: {event}\n\n")
    };
    [
        chunk(
            json!([{"index": 0, "delta": {"role": "assistant", "content": text}, "finish_reason": null}]),
            Value::Null,
        ),
        chunk(
            json!([{"index": 0, "delta": {}, "finish_reason": "stop"}]),
            Value::Null,
        ),
        chunk(
            json!([]),
            json!({"prompt_tokens": 12, "completion_tokens": 3, "total_tokens": 15}),
        ),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat()
}
