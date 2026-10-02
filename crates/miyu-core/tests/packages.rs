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
//! [`clear_uploads`] 也在这个文件（施工 W-5）：和 `register` 一样是「核心起来时做一次」的登记、收拾，不走真
//! 核心的接连接那一段，直接调这个函数——真正分块写、写不满就拒这些行为，`crates/miyu-endpoint/tests/uploads.rs`
//! 已经测过了（`blob.open`、`blob.write`、`blob.close`），这里只管「起来时清掉上一回留下的」这一步真的接进了
//! 起来的先后里。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::queries::Queries;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use serde_json::json;
use support::{Head, Home, resources, within};

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
    drop(head);
    running.abort();
}

#[tokio::test]
async fn the_real_core_draws_a_flowchart_and_a_sequence_diagram() {
    let home = Home::new();
    let queries = miyu_core::packages::register(&resources());
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
    let queries = miyu_core::packages::register(&resources());
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
    let queries = miyu_core::packages::register(&resources());
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
    let queries = miyu_core::packages::register(&resources());
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
    let queries = miyu_core::packages::register(&resources());
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
