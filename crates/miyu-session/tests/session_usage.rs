//! 金额和查用量走一遍真会话（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条）：端口照价格算好的金额记进 `model.called`
//! 的 `cost`，落了盘写进用量汇总；她调 `session_usage` 看到的就是这个会话到这时的用量、金额，上下文照内核的估算、窗口、
//! 压缩线。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, Real};
use miyu_kernel::session::Command;
use miyu_models::catalog::{Price, Rates};
use miyu_models::price::Tariff;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::usage::Query;
use miyu_tool::Catalog;

use crate::support::*;

fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 输入 1、输出 2、读缓存 0.5 美元每百万：剧本一次报 60 没命中、40 命中、10 输出，花 0.0001。
fn tariff() -> Tariff {
    Tariff {
        price: Price::of(
            Rates {
                input: Some(1.0),
                output: Some(2.0),
                cache_read: Some(0.5),
                cache_write: None,
            },
            "USD",
        ),
        multiplier: 1.0,
        source: "config:system/config.toml:7".to_string(),
    }
}

/// 说一句，等到结束了 `turns` 轮。
async fn one_turn(home: &Home, handle: &Handle, turns: usize) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "看看".to_string(),
        })],
        urgent: false,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turns}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == turns
    })
    .await
}

/// 最后一次调用交回的字。
fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

#[tokio::test]
async fn the_cost_is_recorded_and_she_reads_her_own_usage() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::calls(&[("session_usage", "{}")]),
        Play::Says("看完了。"),
    ])
    .window(100_000)
    .priced(tariff());
    let handle = home.create_with(&script, &basesystem(&home)).await;
    let first = one_turn(&home, &handle, 1).await;
    let costs: Vec<Real> = first
        .iter()
        .filter_map(|event| match &event.body {
            Body::ModelCalled(called) => called.cost.as_ref().map(|cost| cost.amount),
            _ => None,
        })
        .collect();
    assert_eq!(costs, [Real::new((60.0 + 40.0 * 0.5 + 10.0 * 2.0) / 1e6)]);
    let log = one_turn(&home, &handle, 2).await;
    let text = last_result(&log);
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("Usage so far: 2 requests, 200 input tokens (80 from cache), 20 output tokens.")
    );
    assert_eq!(lines.next(), Some("Cost: 0.0002 USD."));
    let context = lines.next().expect("有上下文");
    assert!(
        context.starts_with("Context: ") && context.ends_with(" of 100000 tokens."),
        "{text}"
    );
    assert_eq!(lines.next(), Some("Compaction starts at 67000."));
    assert_eq!(lines.next(), None, "{text}");
    // 用量汇总里照日志记着三次（看完了的那一次也落了盘）：和会话写的是同一个连接。
    let query = Query {
        from: None,
        until: None,
        group: Vec::new(),
        session: Some((handle.id().clone(), false)),
        offset: miyu_kernel::time::UtcOffset::UTC,
    };
    // 汇总是会话在 `model.called` 落了盘以后、另起阻塞线程写的：看完结果的那一次可能还没写进去，等它写完再比
    // （慢的 CI 机器上撞过，2026-10-02）。最多等一分钟，断言结果不断言耗时。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let totals = loop {
        let totals = home.usage.query(&query).expect("查得了");
        if totals.first().is_some_and(|row| row.requests >= 3)
            || std::time::Instant::now() > deadline
        {
            break totals;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    assert_eq!(totals[0].requests, 3);
    assert_eq!(totals[0].unpriced, 0);
}
