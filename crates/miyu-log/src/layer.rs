//! 把一条事件写成一行（`docs/designs/28-运行日志.md` 第二节），交给一个 [`Sink`]：平时是按大小轮换的
//! 文件，测试里是 [`Memory`]。
//!
//! 会话编号跟着 span 走：会话 actor 开一个带 `session` 的 span，底下 HTTP 发的行也带上它，写在来源
//! 后面；套了几层的，用离得最近的那一层。事件自己带了 `session` 的，用事件的。
//!
//! 会话的 span 开在 `ERROR` 级（`tracing::error_span!`）：span 也照级别筛，开在 `INFO` 的话，调到
//! `WARN` 它就被筛掉了，底下的行就没了会话编号。
//!
//! 这件事和每个值里的家目录写成 `~`（施工 4-9 再补四上）：先换，再加引号、转义。

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::home::Home;
use crate::line::{self, Parts};

/// 写一行的地方。
pub trait Sink: Send + Sync {
    /// 写一行，末尾的换行由它加。
    fn write_line(&self, line: &str);
}

/// 测试用的：写进来的行都留在内存里。
#[derive(Debug, Default)]
pub struct Memory {
    lines: Mutex<Vec<String>>,
}

impl Memory {
    /// 一个空的。
    pub fn new() -> Arc<Memory> {
        Arc::new(Memory::default())
    }

    /// 到现在写进来的，照先后。
    pub fn lines(&self) -> Vec<String> {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Sink for Memory {
    fn write_line(&self, line: &str) {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line.to_string());
    }
}

/// 写成一行的那一层。
pub struct LineLayer {
    sink: Arc<dyn Sink>,
    clock: fn() -> String,
    home: Home,
}

impl fmt::Debug for LineLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineLayer").finish_non_exhaustive()
    }
}

impl LineLayer {
    /// 写进 `sink`，时刻取本机时间，家目录不换。
    pub fn new(sink: Arc<dyn Sink>) -> LineLayer {
        LineLayer::with_clock(sink, line::now)
    }

    /// 时刻照 `clock` 给的：测试里定住它。
    pub fn with_clock(sink: Arc<dyn Sink>, clock: fn() -> String) -> LineLayer {
        LineLayer {
            sink,
            clock,
            home: Home::default(),
        }
    }

    /// 这件事和每个值里的家目录 `home` 写成 `~`；没有的不换。
    pub fn home(self, home: Option<&Path>) -> LineLayer {
        LineLayer {
            home: Home::new(home),
            ..self
        }
    }
}

/// 一个 span 记下的字段。
#[derive(Debug, Default)]
struct Fields(Vec<(&'static str, String)>);

impl Fields {
    fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name(), value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.push((field.name(), format!("{value:?}")));
    }
}

impl<S> Layer<S> for LineLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(fields) = span.extensions_mut().get_mut::<Fields>()
        {
            values.record(fields);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let message = fields
            .0
            .iter()
            .position(|(key, _)| *key == "message")
            .map(|index| fields.0.remove(index).1)
            .unwrap_or_default();
        let message = self.home.shorten(&message);
        for (_, value) in &mut fields.0 {
            *value = self.home.shorten(value);
        }
        let mut session = fields
            .0
            .iter()
            .position(|(key, _)| *key == "session")
            .map(|index| fields.0.remove(index).1);
        if session.is_none()
            && let Some(scope) = ctx.event_scope(event)
        {
            session = scope.into_iter().find_map(|span| {
                span.extensions()
                    .get::<Fields>()
                    .and_then(|fields| fields.get("session").map(str::to_string))
            });
        }
        let metadata = event.metadata();
        let time = (self.clock)();
        let line = line::format(&Parts {
            time: &time,
            level: metadata.level(),
            source: line::source(metadata.target()),
            session: session.as_deref(),
            message: &message,
            fields: &fields.0,
        });
        self.sink.write_line(&line);
    }
}

#[cfg(test)]
mod tests;
