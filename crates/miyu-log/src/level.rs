//! `MIYU_LOG` 的值怎么读（`docs/designs/28-运行日志.md` 第三节，LG2）：`error`、`warn`、`info`、
//! `debug`、`trace`、`off`，不分大小写；没设、空的是 `info`；读不懂的也照 `info`，交回读不懂的值，
//! 由装日志的地方记一条 `WARN`。配置项 `log.level` 等配置那一步。

use tracing_subscriber::filter::LevelFilter;

/// 读出来的级别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    /// 记到哪一级。
    pub filter: LevelFilter,
    /// 读不懂的原值；读懂了的没有。
    pub unknown: Option<String>,
}

/// 读 `MIYU_LOG` 的值。
pub fn level(value: Option<&str>) -> Level {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Level {
            filter: LevelFilter::INFO,
            unknown: None,
        };
    };
    let filter = match text.to_ascii_lowercase().as_str() {
        "error" => Some(LevelFilter::ERROR),
        "warn" => Some(LevelFilter::WARN),
        "info" => Some(LevelFilter::INFO),
        "debug" => Some(LevelFilter::DEBUG),
        "trace" => Some(LevelFilter::TRACE),
        "off" => Some(LevelFilter::OFF),
        _ => None,
    };
    Level {
        filter: filter.unwrap_or(LevelFilter::INFO),
        unknown: filter.is_none().then(|| text.to_string()),
    }
}

#[cfg(test)]
mod tests;
