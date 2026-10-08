//! 会话的路由（`docs/blueprint/models.md`「守着它的」`route.rs` 那一行 8-6 的一半，施工 8-6）：假服务器收到这一家的 key（施工
//! 8-25：一家一个 key），重启还是它；老的 `keys` 列表不认；取不到 key、没配 `models.chat` 的当场 `no_model`，不发；没写 key 的不带认证头；
//! 会话钉着造它时的模型，`models.chat` 改了只影响新会话；造的时候没配的，配好以后下一轮就用上；窗口照配置。地址是环境变量
//! 的引用时（施工 8-6b）：设了照它连，没设当场 `no_model`，和取不到 key 一样。

use std::sync::Arc;

use tokio::sync::watch;

use crate::support::routing::{called, configs, hellos, items, routes, turn};
use crate::support::{Home, stop};
use miyu_config::Layer;
use miyu_config::merge::{Layers, merge};
use miyu_config::parse::parse;
use miyu_config::problem::Code;
use miyu_config::secret::Reference;
use miyu_http::testkit::Server;
use miyu_kernel::event::ErrorClass;
use miyu_session::{ConfigSource, Handle};

/// 一家 `a` 在 `base_url`，有 key 的照 `{ env = "K1" }` 写，`models.chat` 是 `a/m`。
fn provider(base_url: &str, key: bool) -> String {
    let key = match key {
        true => "key = { env = \"K1\" }\n",
        false => "",
    };
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\n{key}\n[models]\nchat = \"a/m\"\n"
    )
}

/// 环境变量 `K<n>` 的值是 `sk-<n>`，只给 `set` 里的几个。
fn set(set: &[usize]) -> Vec<(Reference, &'static str)> {
    const VALUES: [&str; 4] = ["sk-1", "sk-2", "sk-3", "sk-4"];
    set.iter()
        .map(|n| (Reference::Env(format!("K{n}")), VALUES[n - 1]))
        .collect()
}

/// 服务器收到的第 `n` 个请求的认证头。
fn bearer(server: &Server, n: usize) -> Option<String> {
    server.received()[n]
        .header("authorization")
        .map(str::to_string)
}

fn routes_plain() -> miyu_session::Routes {
    routes(serde_json::json!({}), std::time::Duration::from_secs(60))
}

#[tokio::test]
async fn the_one_key_is_sent_and_still_after_a_restart() {
    let server = Server::start(hellos(4)).await;
    let mut home = Home::new();
    home.configs = configs(&provider(&server.base_url, true), &set(&[1]));
    let handle = home.create(&routes_plain()).await;
    turn(&handle, "cmd-1").await;
    assert!(!server.received().is_empty());
    for at in 0..server.received().len() {
        assert_eq!(bearer(&server, at), Some("Bearer sk-1".to_string()));
    }
    // 重启：停掉再载入，换一个新的路由、一台新的服务器，还是它。
    let session = handle.id().clone();
    stop(&handle).await;
    let again = Server::start(hellos(4)).await;
    home.configs = configs(&provider(&again.base_url, true), &set(&[1]));
    let handle = home.load(&session, &routes_plain()).await;
    turn(&handle, "cmd-again").await;
    assert_eq!(bearer(&again, 0), Some("Bearer sk-1".to_string()));
    let calls = called(&home, &handle);
    assert!(calls.len() >= 2);
    for call in calls {
        assert_eq!(
            (
                call.endpoint.as_ref().map(|e| e.as_str()),
                call.model.as_ref().map(|m| m.as_str())
            ),
            (Some("a"), Some("m")),
            "记进 model.called 的是这一家、这个模型"
        );
    }
}

/// 一家一个 key（施工 8-25）：老的 `keys` 列表不认，报不认识的键，最终值里没有它（这一家照没写 key 办）。
#[test]
fn an_old_keys_list_is_not_read() {
    let parsed = parse(
        &items(),
        Layer::System,
        "[providers.a]\ndriver = \"openai-chat\"\nkeys = [{ env = \"K1\" }]\n",
    )
    .expect("写法对");
    let found: Vec<(Code, Option<&str>)> = parsed
        .problems
        .iter()
        .map(|problem| (problem.code, problem.key.as_deref()))
        .collect();
    assert_eq!(found, [(Code::UnknownKey, Some("providers.a.keys"))]);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    let values = merge(&items(), &layers, &|_| None).values();
    assert!(
        values.keys().all(|key| !key.ends_with(".keys")),
        "{values:?}"
    );
}

#[tokio::test]
async fn no_model_is_said_at_once_and_nothing_is_sent() {
    let server = Server::start(hellos(1)).await;
    for (source, secrets, why) in [
        (
            String::new(),
            set(&[]),
            "no model configured: set models.chat",
        ),
        (
            provider(&server.base_url, true),
            set(&[]),
            r#"provider "a" has no usable key"#,
        ),
        (
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = { env = \"RELAY_URL\" }\n\n[models]\nchat = \"a/m\"\n"
                .to_string(),
            set(&[]),
            r#"provider "a" has no usable base_url"#,
        ),
        (
            "[models]\nchat = \"b/m\"\n".to_string(),
            set(&[]),
            r#"no provider "b""#,
        ),
        (
            "[providers.x]\nlocal = true\n\n[models]\nchat = \"x/m\"\n".to_string(),
            set(&[]),
            r#"provider "x" needs base_url: it matches nothing in the catalog"#,
        ),
    ] {
        let mut home = Home::new();
        home.configs = configs(&source, &secrets);
        let handle = home.create(&routes_plain()).await;
        turn(&handle, "cmd-1").await;
        let calls = called(&home, &handle);
        assert_eq!(calls.len(), 1, "不再来：{why}");
        let error = calls[0].error.as_ref().expect("出错");
        assert_eq!(
            (error.class.clone(), error.message.as_str()),
            (ErrorClass::NoModel, why)
        );
        assert_eq!(calls[0].endpoint, None, "没发出去");
        assert_eq!(calls[0].request, None);
    }
    assert!(server.received().is_empty(), "一个请求都没发");
}

#[tokio::test]
async fn a_provider_without_keys_sends_no_auth_header() {
    let server = Server::start(hellos(1)).await;
    let mut home = Home::new();
    home.configs = configs(&provider(&server.base_url, false), &[]);
    let handle = home.create(&routes_plain()).await;
    turn(&handle, "cmd-1").await;
    assert_eq!(server.received().len(), 1);
    assert_eq!(bearer(&server, 0), None);
}

/// 地址是环境变量的引用（施工 8-6b）：设了就照它连，和写死的地址一样发得出去。
#[tokio::test]
async fn an_address_from_the_environment_connects_like_a_literal_one() {
    let server = Server::start(hellos(1)).await;
    let mut home = Home::new();
    let source = "[providers.a]\ndriver = \"openai-chat\"\nbase_url = { env = \"RELAY_URL\" }\nkey = { env = \"K1\" }\n\n[models]\nchat = \"a/m\"\n";
    home.configs = configs(
        source,
        &[
            (
                Reference::Env("RELAY_URL".to_string()),
                server.base_url.as_str(),
            ),
            (Reference::Env("K1".to_string()), "sk-1"),
        ],
    );
    let handle = home.create(&routes_plain()).await;
    turn(&handle, "cmd-1").await;
    assert_eq!(server.received().len(), 1, "连上了假服务器");
    assert_eq!(bearer(&server, 0), Some("Bearer sk-1".to_string()));
}

#[tokio::test]
async fn a_session_keeps_its_model_and_a_late_chat_is_picked_up() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let both = |chat: &str| {
        format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"{chat}\"\n",
            first.base_url, second.base_url
        )
    };
    let source =
        |text: &str| -> Arc<dyn ConfigSource> { Arc::clone(&*configs(text, &[]).borrow()) };
    let (switch, receiving) = watch::channel(source(&both("a/m")));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes_plain();
    let old = home.create(&routes).await;
    turn(&old, "cmd-1").await;
    // 改成 b：开着的会话还在 a 上（`models.chat` 是 new_session），新会话去 b。
    switch.send_replace(source(&both("b/m")));
    turn(&old, "cmd-2").await;
    let new = home.create(&routes).await;
    turn(&new, "cmd-3").await;
    let endpoints = |handle: &Handle| -> Vec<Option<String>> {
        called(&home, handle)
            .iter()
            .map(|call| call.endpoint.as_ref().map(|e| e.as_str().to_string()))
            .collect()
    };
    let all = |handle: &Handle, wanted: &str| {
        endpoints(handle)
            .iter()
            .all(|e| e.as_deref() == Some(wanted))
    };
    assert!(all(&old, "a"), "{:?}", endpoints(&old));
    assert!(all(&new, "b"), "{:?}", endpoints(&new));

    // 造的时候什么都没配：先是 no_model；配好以后下一轮就用上。
    switch.send_replace(source(""));
    let late = home.create(&routes).await;
    turn(&late, "cmd-4").await;
    switch.send_replace(source(&both("b/m")));
    turn(&late, "cmd-5").await;
    let calls = called(&home, &late);
    assert_eq!(
        calls[..2]
            .iter()
            .map(|call| call.error.as_ref().map(|e| e.class.clone()))
            .collect::<Vec<_>>(),
        [Some(ErrorClass::NoModel), None]
    );
    assert_eq!(endpoints(&late)[..2], [None, Some("b".to_string())]);
}

#[tokio::test]
async fn the_window_comes_from_the_config() {
    let mut home = Home::new();
    home.configs = configs(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"http://unused.invalid\"\n\n[providers.a.models.\"m.1\"]\nwindow = 64000\n\n[models]\nchat = \"a/m.1\"\n",
        &[],
    );
    let handle = home.create(&routes_plain()).await;
    assert_eq!(handle.limits().window, Some(64_000));
    let none = Home::new();
    let handle = none.create(&routes_plain()).await;
    assert_eq!(handle.limits().window, None, "没配模型的不主动压");
}

#[tokio::test]
async fn a_pinned_model_that_is_gone_falls_back_and_stays() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let provider = |id: &str, server: &Server| {
        format!(
            "[providers.{id}]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n",
            server.base_url
        )
    };
    let source =
        |text: String| -> Arc<dyn ConfigSource> { Arc::clone(&*configs(&text, &[]).borrow()) };
    let both = format!(
        "{}{}[models]\nchat = \"a/m\"\n",
        provider("a", &first),
        provider("b", &second)
    );
    let (switch, receiving) = watch::channel(source(both.clone()));
    let mut home = Home::new();
    home.configs = receiving;
    let handle = home.create(&routes_plain()).await;
    turn(&handle, "cmd-1").await;
    // `a` 没了：钉着的解析不出，退回这一轮的 `models.chat`（b），以后钉在它上面。
    switch.send_replace(source(format!(
        "{}[models]\nchat = \"b/m\"\n",
        provider("b", &second)
    )));
    turn(&handle, "cmd-2").await;
    switch.send_replace(source(both));
    turn(&handle, "cmd-3").await;
    let endpoints: Vec<Option<String>> = called(&home, &handle)
        .iter()
        .filter(|call| call.purpose.is_none())
        .map(|call| call.endpoint.as_ref().map(|e| e.as_str().to_string()))
        .collect();
    assert_eq!(
        endpoints,
        [
            Some("a".to_string()),
            Some("b".to_string()),
            Some("b".to_string())
        ]
    );
}
