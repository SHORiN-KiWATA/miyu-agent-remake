//! 「终端管理员与白名单成员」页（施工 O-17，`onebot.md` 第二条「怎么走」第 4 条、「施工时定的」第 26 到 36 条）：真的核心加真的桥，
//! 浏览器经桥的 `/ws` 照页面（`people.js`）的样子发。还没设好密码的连接改系统配置被拒、什么都没写（核心现在只有管理员一个
//! 账号，第 35 条）；设好以后加一个终端管理员、删一个终端管理员（恢复默认）、整张写回白名单成员，换一条连接 `config.get` 读得回；新终端管理员的
//! 私聊当场认得是管理员本人。白名单成员写进个人设置、写成一个字、元素是空的字，整条不收、什么都没变。

use futures_util::SinkExt;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use miyu_session::testkit::{Play, Script};

use crate::support::http::*;
use crate::support::*;

/// 页面加的第二个终端管理员（终端管理员的小号）。
const SECOND: i64 = 10_002;

/// 终端管理员对应表里一格的键，照页面拼：`external.bindings."qq:<号>"`。
fn binding(number: i64) -> String {
    format!("external.bindings.\"qq:{number}\"")
}

/// 一条经桥的 `/ws` 连着核心的浏览器，照页面的样子发请求、等回应。
struct Page {
    ws: Browser,
    sent: u64,
}

impl Page {
    /// 照浏览器的样子连桥 `web` 端口上的 `/ws`（Origin 是页面自己的地址），握手带 `credentials`，交回连接和握手的回应。
    async fn open(web: u16, credentials: Value) -> (Page, Value) {
        let origin = format!("http://127.0.0.1:{web}");
        let ws = browser(web, Some(&origin)).await.expect("连得上");
        let mut page = Page { ws, sent: 0 };
        let mut params = json!({
            "protocol": [1, 1],
            "head": {"kind": "onebot-web", "version": "0"},
            "locale": "zh-CN",
        });
        if let (Some(params), Some(credentials)) = (params.as_object_mut(), credentials.as_object())
        {
            params.extend(credentials.clone());
        }
        let hello = page.call("hello", params).await;
        (page, hello)
    }

    /// 发一条请求，交回整条回应（带 `result` 或 `error`）；核心的推送跳过。
    async fn call(&mut self, method: &str, params: Value) -> Value {
        self.sent += 1;
        let id = format!("onebot-web-test-{}", self.sent);
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.ws
            .send(Message::text(request.to_string()))
            .await
            .expect("发得出");
        loop {
            let Some(Message::Text(text)) = next(&mut self.ws).await else {
                panic!("等 {method} 的回应时连接断了");
            };
            let reply: Value = serde_json::from_str(text.as_str()).expect("是 JSON");
            if reply["id"] == id {
                return reply;
            }
        }
    }

    /// 页面存表：`config.set` 只写系统配置，`changes` 照页面算的。
    async fn save(&mut self, changes: Value) -> Value {
        self.call("config.set", json!({"layer": "system", "changes": changes}))
            .await
    }

    /// 页面读配置：`config.get` 不写 `keys`（全部），交回 `items`。
    async fn items(&mut self) -> Value {
        let reply = self.call("config.get", json!({})).await;
        reply["result"]["items"].clone()
    }
}

/// 拒绝的原因码。
fn why(reply: &Value) -> &str {
    reply["error"]["data"]["reason"].as_str().unwrap_or("")
}

/// `items` 里终端管理员对应表的每一格：键到账号，照键排。
fn admins(items: &Value) -> Vec<(String, Value)> {
    items
        .as_object()
        .expect("是对象")
        .iter()
        .filter(|(key, _)| key.starts_with("external.bindings."))
        .map(|(key, item)| (key.clone(), item["value"].clone()))
        .collect()
}

/// 照页面第一次进门的样子：本机要一个一次性码（`miyu-onebot web` 那样），浏览器带着它经桥连上核心。交回这条还没设好密码的
/// 连接。
async fn with_code(home: &Home, web: u16) -> Page {
    let mut local = within(
        "连上核心",
        miyu_webserve::open::Core::connect_running(&home.root, "test"),
    )
    .await
    .expect("连得上核心");
    let issued = local
        .call("c", "account.setup_code", json!({}))
        .await
        .expect("要得到一次性码");
    let (page, hello) = Page::open(web, json!({"code": issued["code"]})).await;
    assert_eq!(hello["result"]["setup"], true, "{hello}");
    page
}

/// 设好用户名和密码（`account.setup`），交回登录令牌；这条连接从此是管理员。
async fn set_up(page: &mut Page) -> String {
    let done = page
        .call(
            "account.setup",
            json!({"username": "shorin", "password": "correct horse"}),
        )
        .await;
    done["result"]["login"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("设好了有登录令牌：{done}"))
        .to_string()
}

#[tokio::test]
async fn the_page_adds_and_removes_admins_and_writes_the_whitelist_back_whole() {
    let script = Script::new([Play::Says("在。")]);
    let home = Home::new(&script);
    let bridge = bridge(&home).await;
    let mut page = with_code(&home, bridge.web).await;
    // 还没设好密码的连接（核心里还算不上管理员）改系统配置：照原话拒，什么都没写。
    let refused = page
        .save(json!([{"key": binding(SECOND), "value": "admin"}]))
        .await;
    assert_eq!(why(&refused), "setup_first", "{refused}");
    assert!(
        refused["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "页面照原话说：{refused}"
    );
    let login = set_up(&mut page).await;
    assert_eq!(
        admins(&page.items().await),
        [(binding(ADMIN), json!("admin"))],
        "被拒的那一条什么都没写"
    );
    // 加一个终端管理员：照页面拼的键写进系统配置。
    let added = page
        .save(json!([{"key": binding(SECOND), "value": "admin"}]))
        .await;
    assert_eq!(
        added["result"]["keys"][binding(SECOND)]["applies"],
        "now",
        "{added}"
    );
    // 删一个终端管理员：恢复默认，去掉这一格。
    let removed = page
        .save(json!([{"key": binding(ADMIN), "unset": true}]))
        .await;
    assert!(
        removed["result"]["keys"][binding(ADMIN)].is_object(),
        "{removed}"
    );
    // 白名单成员整张写回。
    let whitelist = json!(["qq:20017", format!("qq:{SECOND}")]);
    let written = page
        .save(json!([{"key": "onebot.whitelist", "value": whitelist}]))
        .await;
    assert_eq!(
        written["result"]["keys"]["onebot.whitelist"]["applies"], "now",
        "{written}"
    );
    // 换一条连接（记住的登录令牌），读得回。
    let (mut again, hello) = Page::open(bridge.web, json!({"login": login})).await;
    assert_eq!(hello["result"]["account"], "admin", "{hello}");
    let items = again.items().await;
    assert_eq!(admins(&items), [(binding(SECOND), json!("admin"))]);
    assert_eq!(items["onebot.whitelist"]["value"], whitelist);
    assert_eq!(items["onebot.whitelist"]["origin"]["layer"], "system");
    // 新终端管理员的私聊当场认得是管理员本人：页面拼的键和桥拼的平台身份对得上。
    let mut napcat = admin_napcat(bridge.port).await;
    napcat
        .private(
            SECOND,
            601,
            json!([{"type": "text", "data": {"text": "小号来了"}}]),
        )
        .await;
    let action = napcat.action().await;
    assert_eq!(action["action"], "send_private_msg", "{action}");
    assert_eq!(action["params"]["user_id"], SECOND, "{action}");
    let said = home.said();
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(
        said[0]["by"],
        json!({"kind": "person", "account": "admin", "via": format!("qq:{SECOND}")})
    );
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_whitelist_written_wrong_or_to_the_personal_layer_is_refused_whole() {
    let home = Home::new(&Script::new([]));
    let bridge = bridge(&home).await;
    let mut page = with_code(&home, bridge.web).await;
    set_up(&mut page).await;
    let personal = page
        .call(
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "onebot.whitelist", "value": ["qq:20017"]}]}),
        )
        .await;
    assert_eq!(why(&personal), "config_invalid", "{personal}");
    assert_eq!(
        personal["error"]["data"]["problems"][0]["code"], "wrong_layer",
        "{personal}"
    );
    for wrong in [json!("qq:20017"), json!([""]), json!([20017])] {
        let refused = page
            .save(json!([
                {"key": binding(SECOND), "value": "admin"},
                {"key": "onebot.whitelist", "value": wrong},
            ]))
            .await;
        assert_eq!(why(&refused), "config_invalid", "{wrong}: {refused}");
        let problems = refused["error"]["data"]["problems"]
            .as_array()
            .expect("有问题");
        assert!(
            problems
                .iter()
                .all(|problem| problem["key"] == "onebot.whitelist"
                    && problem["message"]
                        .as_str()
                        .is_some_and(|message| !message.is_empty())),
            "问题说在白名单成员那一项上、带一句给人看的话：{refused}"
        );
    }
    let items = page.items().await;
    assert!(items.get("onebot.whitelist").is_none(), "{items}");
    assert_eq!(
        admins(&items),
        [(binding(ADMIN), json!("admin"))],
        "一起发的终端管理员也没写：整条不收"
    );
    bridge.stop().await.expect("停得下");
}
