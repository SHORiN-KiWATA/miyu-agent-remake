//! 预算表的读法、超没超；仓库里那一份读得出量尺要的几项。

use super::*;

const DESIGN: &str = "\
### 一、怎么量

| 不是 | 这张 |
|---|---|
| 冷启动到能打字 | 1 ms |

### 二、预算

| 项目 | 预算（初值） | 为什么是这个数 |
|---|---|---|
| 冷启动到能打字 | 150 ms | 一两百毫秒以内的等待，人不大察觉 |
| 按键到上屏 | p50 5 ms | jcode 实测 3.5 ms |
| 切会话 | 一帧之内（p95 16 ms） | 要近乎秒切 |
| 追加一步的事件并同步 | p99 20 ms（固态硬盘） | 先落盘后推送 |
| 核心空闲 | 30 MB | 常驻时这是底 |
| 重型 worker | 用完就退，不算常驻 | 只在用的时候占 |

- 超了预算，先查是不是量法的问题。
";

fn ms(quantile: u8, value: f64) -> Budget {
    Budget {
        quantile,
        value,
        unit: Unit::Ms,
    }
}

#[test]
fn the_second_section_is_read() {
    let budgets = parse(DESIGN).unwrap();
    assert_eq!(budgets["冷启动到能打字"], ms(50, 150.0));
    assert_eq!(budgets["按键到上屏"], ms(50, 5.0));
    assert_eq!(budgets["切会话"], ms(95, 16.0));
    assert_eq!(budgets["追加一步的事件并同步"], ms(99, 20.0));
    assert_eq!(
        budgets["核心空闲"],
        Budget {
            quantile: 50,
            value: 30.0,
            unit: Unit::Mb,
        }
    );
    assert!(!budgets.contains_key("重型 worker"));
    assert!(!budgets.contains_key("项目"));
    assert_eq!(budgets.len(), 5);
}

#[test]
fn a_missing_section_or_table_is_an_error() {
    assert!(parse("### 一、怎么量\n").is_err());
    assert!(parse("### 二、预算\n\n没有表。\n").is_err());
}

#[test]
fn over_is_over() {
    let budget = ms(95, 5.0);
    assert_eq!(verdict(Some(4.9), &budget), "过");
    assert_eq!(verdict(Some(5.0), &budget), "过");
    assert_eq!(verdict(Some(5.1), &budget), "超");
    assert_eq!(verdict(None, &budget), "不量");
    assert_eq!(budget.show(), "p95 5 ms");
    assert_eq!(
        Budget {
            quantile: 50,
            value: 30.0,
            unit: Unit::Mb,
        }
        .show(),
        "30 MB"
    );
}

/// 仓库里的 23 读得出量尺比的每一项（名字对不上，量尺跑到最后才报错）。
#[test]
fn the_real_design_has_every_item() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/designs/23-性能预算.md"
    );
    let budgets = parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    for item in crate::report::ITEMS {
        assert!(budgets.contains_key(*item), "23 第二节没有「{item}」");
    }
}
