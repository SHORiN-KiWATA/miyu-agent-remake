//! `session_usage`（`docs/blueprint/tools/session_usage.md`，施工 8-15）：零参数、访问类别读；输出逐字节比：用量总有，金额、
//! 没价格的几次有才写，上下文有窗口的写几成和压缩线（没有压缩线的不写那一句），没窗口的写大约多少，算不出的不写；参数写了
//! 什么都不认；没有端口的照什么都没花答；查不了的出错；叫停。给人看的说法中文、英文都换得出字。

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::tool::Access;
use miyu_tool::{ContextUse, Done, Spending, Spent, Stop, UsagePort};

use support::{Site, check, human, readable, said, tool};

/// 假的查用量端口：交回给的那一份，数着被问了几次。
struct Port {
    spent: Result<Spent, String>,
    context: Option<ContextUse>,
    asked: AtomicUsize,
}

impl UsagePort for Port {
    fn context(&self) -> Option<ContextUse> {
        self.context
    }

    fn spent(&self) -> Spending<'_> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        let spent = self.spent.clone();
        Box::pin(async move { spent })
    }
}

/// 花了的：12 次请求，两种币种（排好了先后），两次没价格。
fn spent() -> Spent {
    Spent {
        requests: 12,
        input: 48_210,
        cached: 40_122,
        output: 3_120,
        amounts: vec![("USD".to_string(), 0.0123), ("CNY".to_string(), 1.3)],
        unpriced: 2,
    }
}

fn port(spent: Result<Spent, String>, context: Option<ContextUse>) -> Arc<Port> {
    Arc::new(Port {
        spent,
        context,
        asked: AtomicUsize::new(0),
    })
}

fn context(used: u64, window: Option<u64>, line: Option<u64>) -> Option<ContextUse> {
    Some(ContextUse { used, window, line })
}

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

async fn usage(port: &Arc<Port>, args: serde_json::Value) -> Done {
    let port = Arc::clone(port) as Arc<dyn UsagePort>;
    Site::new()
        .done_with_usage("session_usage", args, Some(port), Stop::default())
        .await
}

#[test]
fn it_reads_and_takes_no_parameters() {
    let spec = tool("session_usage").spec().clone();
    assert_eq!(spec.access, Access::Read);
    assert_eq!(
        spec.parameters.get(),
        r#"{"type":"object","properties":{}}"#
    );
}

#[tokio::test]
async fn everything_it_knows_one_sentence_a_line() {
    let mut checked = Vec::new();
    let port = port(Ok(spent()), context(23_110, Some(128_000), Some(95_000)));
    let done = usage(&port, json!({"whatever": 1})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "Usage so far: 12 requests, 48210 input tokens (40122 from cache), 3120 output tokens.\n\
         Cost: 0.0123 USD + 1.30 CNY.\n\
         2 requests have no price, so the cost leaves them out.\n\
         Context: 23110 of 128000 tokens.\n\
         Compaction starts at 95000.\n"
    );
    assert_eq!(port.asked.load(Ordering::Relaxed), 1);
    check(&mut checked, human(done), said("session_usage/shown"));
    readable(&checked, &["session_usage"]);
}

#[tokio::test]
async fn only_what_there_is() {
    let plain = Spent {
        amounts: Vec::new(),
        unpriced: 0,
        ..spent()
    };
    let cases = [
        (
            context(900, Some(32_000), None),
            "Context: 900 of 32000 tokens.\n",
        ),
        (
            context(900, None, None),
            "Context: about 900 tokens. This model reports no window.\n",
        ),
        (None, ""),
    ];
    for (context, tail) in cases {
        let done = usage(&port(Ok(plain.clone()), context), json!({})).await;
        assert_eq!(
            text(&done),
            format!(
                "Usage so far: 12 requests, 48210 input tokens (40122 from cache), 3120 output tokens.\n{tail}"
            )
        );
    }
    let tiny = Spent {
        amounts: vec![("USD".to_string(), 0.000_292_05), ("EUR".to_string(), 0.42)],
        ..plain
    };
    let done = usage(&port(Ok(tiny), None), json!({})).await;
    assert!(
        text(&done).contains("Cost: 0.000292 USD + 0.42 EUR.\n"),
        "{}",
        text(&done)
    );
}

#[tokio::test]
async fn without_a_port_nothing_was_spent() {
    let done = Site::new()
        .done_with_usage("session_usage", json!({}), None, Stop::default())
        .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "Usage so far: 0 requests, 0 input tokens (0 from cache), 0 output tokens.\n"
    );
}

#[tokio::test]
async fn a_failure_and_stopping() {
    let mut checked = Vec::new();
    let failing = port(Err("database is locked".to_string()), None);
    let done = usage(&failing, json!({})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Could not read the usage: database is locked\n"
    );
    check(
        &mut checked,
        human(done),
        said("session_usage/failed").with("error", "database is locked"),
    );
    readable(&checked, &[]);
    let stop = Stop::default();
    stop.raise();
    let port = port(Ok(spent()), None) as Arc<dyn UsagePort>;
    let done = Site::new()
        .done_with_usage("session_usage", json!({}), Some(port), stop)
        .await;
    assert!(done.stopped, "叫停了的停下");
}
