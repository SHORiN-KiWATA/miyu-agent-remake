//! `session_usage`（`docs/blueprint/tools/session_usage.md`，施工 8-15）：她自己查这个会话花了多少、上下文还剩多少。零参数，
//! 访问类别是读。经 [`Call::usage`] 交给执行器：用量、金额照用量汇总（只算这个会话、不带子会话，和头读的是同一份），上下文是
//! 派出去那一刻内核照压缩线的算法估的。
//!
//! 输出一句一行：用量总有；有金额的写金额（照币种各写一段，用 ` + ` 接起来，排好了先后）；有没价格的写几次；算得出上下文的
//! 写上下文，有窗口的写几成、有压缩线的另写一句压缩线，没窗口的写大约多少。金额三位有效数字、至少两位小数：一次请求花的
//! 常常不到一分钱（「施工时定的」8-15）。

use std::path::Path;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, ContextUse, Done, Progress, Running, SESSION_USAGE, Spec, Spent, Tool};

use crate::common::said;
use crate::load::{self, LoadError, say};

/// `session_usage`。
pub(crate) struct SessionUsage {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/session_usage/*.txt`。
#[derive(Clone)]
struct Texts {
    usage: Template,
    cost: Template,
    unpriced: Template,
    context: Template,
    compaction: Template,
    no_window: Template,
    failed: Template,
}

impl SessionUsage {
    /// 照资源目录 `resources` 里的字造。访问类别是读：只读用量汇总、会话的日志，什么都不改。
    pub(crate) fn load(resources: &Path) -> Result<SessionUsage, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, SESSION_USAGE, name, fields);
        Ok(SessionUsage {
            spec: load::spec(resources, SESSION_USAGE, Access::Read)?,
            texts: Texts {
                usage: text("usage", &["requests", "input", "cached", "output"])?,
                cost: text("cost", &["amounts"])?,
                unpriced: text("unpriced", &["count"])?,
                context: text("context", &["used", "window"])?,
                compaction: text("compaction", &["line"])?,
                no_window: text("context-no-window", &["used"])?,
                failed: text("failed", &["error"])?,
            },
        })
    }
}

impl Tool for SessionUsage {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            // 参数一个都不认：写了什么都不报错。没有端口的（测试里的假调用）照什么都没花答。
            let Some(port) = call.usage.clone() else {
                return Done::ok(texts.spent(&nothing(), None)).said(said("session_usage/shown"));
            };
            let spent = match port.spent().await {
                Ok(spent) => spent,
                Err(error) => {
                    return Done::error(say(&texts.failed, &[("error", &error)]))
                        .said(said("session_usage/failed").with("error", error));
                }
            };
            if call.stop.stopped() {
                return Done::stopped();
            }
            Done::ok(texts.spent(&spent, port.context())).said(said("session_usage/shown"))
        })
    }
}

impl Texts {
    /// 用量、金额、没价格的几次、上下文，一句一行。
    fn spent(&self, spent: &Spent, context: Option<ContextUse>) -> String {
        let number = |n: u64| n.to_string();
        let mut text = say(
            &self.usage,
            &[
                ("requests", &number(spent.requests)),
                ("input", &number(spent.input)),
                ("cached", &number(spent.cached)),
                ("output", &number(spent.output)),
            ],
        );
        if !spent.amounts.is_empty() {
            let amounts: Vec<String> = spent
                .amounts
                .iter()
                .map(|(currency, amount)| format!("{} {currency}", money(*amount)))
                .collect();
            text.push_str(&say(&self.cost, &[("amounts", &amounts.join(" + "))]));
        }
        if spent.unpriced > 0 {
            text.push_str(&say(&self.unpriced, &[("count", &number(spent.unpriced))]));
        }
        let Some(context) = context else {
            return text;
        };
        let used = number(context.used);
        match context.window {
            Some(window) => {
                text.push_str(&say(
                    &self.context,
                    &[("used", &used), ("window", &number(window))],
                ));
                if let Some(line) = context.line {
                    text.push_str(&say(&self.compaction, &[("line", &number(line))]));
                }
            }
            None => text.push_str(&say(&self.no_window, &[("used", &used)])),
        }
        text
    }
}

/// 什么都没花。
fn nothing() -> Spent {
    Spent {
        requests: 0,
        input: 0,
        cached: 0,
        output: 0,
        amounts: Vec::new(),
        unpriced: 0,
    }
}

/// 金额写成三位有效数字、至少两位小数：`0.42`、`1.30`、`0.000292`、`12.35`。
fn money(amount: f64) -> String {
    let magnitude = if amount > 0.0 {
        amount.log10().floor() as i32
    } else {
        0
    };
    let decimals = (2 - magnitude).max(2) as usize;
    let text = format!("{amount:.decimals$}");
    // 两位以后多出来的 0 去掉：三位有效数字凑出来的 `0.420` 写成 `0.42`。
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let mut fraction = fraction.to_string();
    while fraction.len() > 2 && fraction.ends_with('0') {
        fraction.pop();
    }
    format!("{whole}.{fraction}")
}

#[cfg(test)]
mod tests;
