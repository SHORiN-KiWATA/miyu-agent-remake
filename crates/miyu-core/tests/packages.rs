//! 可选软件包往查询表里登记（`web-module.md`「起草时定的」第 19、20 条，`mermaid.md`，施工 W-4）：真核心走
//! 一遍 `mermaid.render`——一张流程图、一张时序图都出 SVG，回应的 `marks` 和 SVG 里用的三种记号色对得上；
//! 协议上的拒绝（空的、太长、画不出）；查询表本身没登记的方法回 `unknown_method`。
//!
//! 「没编进来（关掉 cargo 开关的核心）回 `unknown_method`」照「施工时定」的办法测查询表本身：不额外编一份关掉
//! `mermaid` 开关的核心（CI 的门禁只跑默认开着全部可选软件包的那一份，另编一份要一次新的 cargo 构建，多一道
//! 门禁没有的流程）；一个方法压根没登记、和这个方法所在的软件包没编进来，端点看到的是同一个结果——查询表
//! 找不到就是 `unknown_method`，这正是 `queries.rs` 的 `Queries::call` 给端点的信号（见
//! `crates/miyu-endpoint/src/queries.rs` 的 `an_unregistered_method_is_not_found`，那边直接测表；这里走一遍
//! 真协议，确认端点真的把它翻成了 `unknown_method`）。
//!
//! `link.preview`（施工 W-7，`net.md`）：登记了的不碰网络就答得出的几种（读不成地址、参数不对）；在后台答的查询
//! 不挡这个连接后面的请求、连接断了跟着停（用两个测试的查询，不连网）。真的抓在 `crates/miyu-net/tests/`，真的核心、
//! 环境变量里的代理在 `crates/miyu/tests/link_preview.rs`。
//!
//! [`clear_uploads`] 也在这个文件（施工 W-5）：和 `register` 一样是「核心起来时做一次」的登记、收拾，不走真
//! 核心的接连接那一段，直接调这个函数——真正分块写、写不满就拒这些行为，`crates/miyu-endpoint/tests/uploads.rs`
//! 已经测过了（`blob.open`、`blob.write`、`blob.close`），这里只管「起来时清掉上一回留下的」这一步真的接进了
//! 起来的先后里。

use std::sync::Arc;
use std::time::Duration;

use crate::support::{Head, Home, resources, within};
use miyu_endpoint::queries::Queries;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use serde_json::json;

/// 测试里的空闲时限：这组测试都自己连着头，不等它空闲退出。
const IDLE: Duration = Duration::from_secs(60);

/// 起一个真核心、接上一个头，交回两样东西：等它跑完的句柄、已经握过手的头。
async fn running_core(
    home: &Home,
    queries: Queries,
) -> (tokio::task::JoinHandle<miyu_core::Stopped>, Head) {
    let opened = home.open();
    let core = home.core_with_queries(Arc::new(Script::new([])), &opened.token, queries);
    let running = tokio::spawn(miyu_core::serve(
        opened.listener,
        core,
        IDLE,
        std::future::pending(),
    ));
    let head = within("连得上", Head::connect(&home.root)).await;
    (running, head)
}

/// 卸掉了画 mermaid、联网这两个出厂的包（家目录记了一笔，施工 F-5 上），编进来了、登记了也当没有 `mermaid.render`、
/// `link.preview`（施工 F-2、F-5 中，设计 30 第二节第 3 条）。
#[tokio::test]
async fn a_query_of_a_package_not_installed_is_unknown_method() {
    let home = Home::new();
    let mine = home.root.account_dir(&miyu_core::admin()).join("packages");
    std::fs::create_dir_all(&mine).expect("建得了");
    for removed in ["mermaid.removed", "net.removed"] {
        std::fs::write(mine.join(removed), "").expect("写得进");
    }
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    let reply = head
        .call(
            "q1",
            "mermaid.render",
            json!({"source": "flowchart TD\nA-->B"}),
        )
        .await;
    assert_eq!(
        reply["error"]["data"]["reason"], "unknown_method",
        "{reply}"
    );
    let reply = head
        .call("q2", "link.preview", json!({"url": "not a url"}))
        .await;
    assert_eq!(
        reply["error"]["data"]["reason"], "unknown_method",
        "{reply}"
    );
    drop(head);
    running.abort();
}

/// 起来时就卸掉了人格记忆（家目录记了一笔）：`memory.*` 回 `memory_not_installed`（施工 R-10，`memory.md` 第十一条）。装卸
/// 当场换的在 `crates/miyu-endpoint/tests/memory_installed.rs`。
#[tokio::test]
async fn memory_removed_before_start_says_not_installed() {
    let home = Home::new();
    let mine = home.root.account_dir(&miyu_core::admin()).join("packages");
    std::fs::create_dir_all(&mine).expect("建得了");
    std::fs::write(mine.join("memory.removed"), "").expect("写得进");
    let (running, mut head) = running_core(&home, Queries::new()).await;
    let reply = head.call("m1", "memory.list", json!({})).await;
    assert_eq!(
        reply["error"]["data"]["reason"], "memory_not_installed",
        "{reply}"
    );
    drop(head);
    running.abort();
}

#[tokio::test]
async fn an_unregistered_method_is_unknown_method() {
    let home = Home::new();
    let (running, mut head) = running_core(&home, Queries::new()).await;
    let reply = head
        .call(
            "q1",
            "mermaid.render",
            json!({"source": "flowchart TD\nA-->B"}),
        )
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "unknown_method");
    let reply = head
        .call("q2", "link.preview", json!({"url": "not a url"}))
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "unknown_method");
    drop(head);
    running.abort();
}

#[tokio::test]
async fn link_preview_is_registered_and_answers_without_the_network() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    for (url, why) in [
        ("not a url", "not_a_url"),
        ("  ftp://example.com/  ", "unsupported_scheme"),
        ("javascript:alert(1)", "unsupported_scheme"),
    ] {
        let reply = head.call("p1", "link.preview", json!({"url": url})).await;
        assert_eq!(reply["result"], json!({"card": null, "why": why}), "{url}");
    }
    for params in [json!({}), json!({"url": 5}), json!(["http://example.com/"])] {
        let reply = head.call("p2", "link.preview", params.clone()).await;
        assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{params}");
    }
    drop(head);
    running.abort();
}

#[tokio::test]
async fn a_background_query_does_not_hold_up_the_next_request() {
    let home = Home::new();
    let gate = Arc::new(tokio::sync::Notify::new());
    let opened = Arc::clone(&gate);
    let queries = Queries::new()
        .register_background("probe.slow", move |_core, _params| {
            let opened = Arc::clone(&opened);
            async move {
                opened.notified().await;
                Ok(json!({"slow": true}))
            }
        })
        .register("probe.fast", |_core, _params| async move {
            Ok(json!({"fast": true}))
        });
    let (running, mut head) = running_core(&home, queries).await;
    head.send("s1", "probe.slow", json!({})).await;
    // 慢的还没答，后面的先回来
    let fast = head.call("f1", "probe.fast", json!({})).await;
    assert_eq!(fast["result"], json!({"fast": true}));
    gate.notify_one();
    let slow = head.next_reply().await;
    assert_eq!(slow["id"], "s1", "回应照 id 对上");
    assert_eq!(slow["result"], json!({"slow": true}));
    drop(head);
    running.abort();
}

/// 丢掉时说一声：在后台答的任务被停下了。
struct Dropped(Option<tokio::sync::oneshot::Sender<()>>);

impl Drop for Dropped {
    fn drop(&mut self) {
        if let Some(said) = self.0.take() {
            let _sent = said.send(());
        }
    }
}

#[tokio::test]
async fn a_background_query_stops_when_its_connection_goes() {
    let home = Home::new();
    let (started_tx, started) = tokio::sync::oneshot::channel::<()>();
    let (dropped_tx, dropped) = tokio::sync::oneshot::channel::<()>();
    let hooks = Arc::new(std::sync::Mutex::new(Some((started_tx, dropped_tx))));
    let queries = Queries::new().register_background("probe.hang", move |_core, _params| {
        let hooks = hooks.lock().unwrap().take();
        async move {
            let (started_tx, dropped_tx) = hooks.expect("只调一次");
            let _guard = Dropped(Some(dropped_tx));
            let _sent = started_tx.send(());
            std::future::pending::<()>().await;
            Ok(json!({}))
        }
    });
    let (running, mut head) = running_core(&home, queries).await;
    head.send("h1", "probe.hang", json!({})).await;
    within("后台的任务开始了", started).await.unwrap();
    drop(head);
    within("连接断了，后台的任务跟着停", dropped).await.unwrap();
    running.abort();
}

#[tokio::test]
async fn the_real_core_draws_a_flowchart_and_a_sequence_diagram() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    for (name, source) in [
        (
            "flow",
            "flowchart TD\n  A[Start] --> B{Decision}\n  B -->|Yes| C[OK]\n  B -->|No| D[Cancel]",
        ),
        (
            "seq",
            "sequenceDiagram\n  participant U as User\n  participant C as Core\n  U->>C: hello\n  C-->>U: ok",
        ),
    ] {
        let reply = head
            .call(name, "mermaid.render", json!({"source": source}))
            .await;
        assert!(reply.get("error").is_none(), "{name}: {reply}");
        let svg = reply["result"]["svg"].as_str().expect("svg 是字符串");
        assert!(svg.starts_with("<svg"), "{name}: {svg}");
        // 回应的 marks 和 SVG 里用的三种记号色对得上。
        for colour in ["text", "line", "label"] {
            let mark = reply["result"]["marks"][colour]
                .as_str()
                .unwrap_or_else(|| panic!("{name}: 没有 marks.{colour}"));
            assert!(
                svg.contains(mark),
                "{name}: SVG 里没有 marks.{colour}={mark}"
            );
        }
    }
    drop(head);
    running.abort();
}

#[tokio::test]
async fn an_empty_source_is_bad_params() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    let reply = head
        .call("q1", "mermaid.render", json!({"source": "   "}))
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "bad_params");
    drop(head);
    running.abort();
}

#[tokio::test]
async fn a_source_over_the_limit_is_mermaid_too_long() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    // 真的 style.json 里 max_source 是 65536（64 KiB）。
    let source = "x".repeat(65537);
    let reply = head
        .call("q1", "mermaid.render", json!({"source": source}))
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "mermaid_too_long");
    drop(head);
    running.abort();
}

#[tokio::test]
async fn an_unparseable_source_is_mermaid_failed_with_the_library_detail() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    let reply = head
        .call(
            "q1",
            "mermaid.render",
            json!({"source": "not a diagram at all"}),
        )
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "mermaid_failed");
    let detail = reply["error"]["data"]["detail"]
        .as_str()
        .expect("detail 是字符串");
    assert!(!detail.is_empty(), "库的原话不是空的");
    drop(head);
    running.abort();
}

#[tokio::test]
async fn the_same_source_twice_gets_the_same_svg() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources(), &home.root, &miyu_core::admin());
    let (running, mut head) = running_core(&home, queries).await;
    let params = json!({"source": "flowchart TD\nA-->B"});
    let first = head.call("q1", "mermaid.render", params.clone()).await;
    let second = head.call("q2", "mermaid.render", params).await;
    assert_eq!(
        first["result"], second["result"],
        "同一份源码画出一样的 SVG"
    );
    drop(head);
    running.abort();
}

#[tokio::test]
async fn startup_clears_leftover_upload_temp_files_but_leaves_blobs_alone() {
    let home = Home::new();
    let admin = miyu_core::admin();
    let blobs = Blobs::new(home.root.blobs(&admin));
    // 崩了、被杀留下的分块上传暂存。
    blobs.create_upload("leftover").unwrap();
    // 一份真的 blob，不该被这一步碰到。
    let kept = blobs.put(b"kept across a restart").unwrap();
    miyu_core::packages::clear_uploads(&home.root, &admin);
    assert!(
        !blobs.upload_path("leftover").exists(),
        "崩溃留下的暂存清掉了"
    );
    assert_eq!(blobs.get(&kept).unwrap(), b"kept across a restart");
}

#[tokio::test]
async fn clearing_uploads_is_fine_when_the_account_never_stored_anything() {
    let home = Home::new();
    // 这个账号的 blobs/ 目录都还没建过：起来时清暂存不该因为「没有」就出错、也不该把它建出来。
    miyu_core::packages::clear_uploads(&home.root, &miyu_core::admin());
    assert!(!home.root.blobs(&miyu_core::admin()).exists());
}
