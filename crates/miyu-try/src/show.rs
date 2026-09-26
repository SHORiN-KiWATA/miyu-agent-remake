//! 往终端写（`docs/construction/3-5-试玩台（补）.md`）：思考和回复边出边写，思考暗色；出了错、要
//! 重试的写一行；每次请求一行用量；这一轮不是正常走完的，写怎么结束的。只写给人看，不进日志。
//!
//! 用量那一行照 `08-上下文投影.md` 第七节看绝对值：命中、没命中、输出；第二次请求起再写上一次发过
//! 的那些，这一次重算了多少。

use std::collections::BTreeMap;
use std::io::{self, Write};

use miyu_kernel::accumulate::Kind;
use miyu_kernel::event::{
    Body, CallError, CallResult, EndReason, ErrorClass, Event, ModelCalled, Piece, Status,
    Transient, TransientBody,
};

/// 暗色和复原：只在写给终端时用。
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// 出错的原话在终端上最多写多少个字：出错页可能是一整页 HTML。
const MESSAGE_CHARS: usize = 160;

/// 往一个地方写。
pub struct Show<W: Write> {
    out: W,
    /// 写给终端的：思考暗色。不是终端的（管道、测试），思考前面写「思考：」。
    color: bool,
    /// 正在出的那次回复，每一块是什么，照块的编号。
    kinds: BTreeMap<usize, Kind>,
    /// 光标在不在行首。
    fresh: bool,
    /// 上一次请求的输入一共多少 token：算这一次重算了多少。
    last_input: Option<u64>,
}

impl<W: Write> Show<W> {
    /// 往 `out` 写；`color` 为真的，思考用暗色。
    pub fn new(out: W, color: bool) -> Show<W> {
        Show {
            out,
            color,
            kinds: BTreeMap::new(),
            fresh: true,
            last_input: None,
        }
    }

    /// 另起一行写一整行。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn line(&mut self, text: &str) -> io::Result<()> {
        self.fresh_line()?;
        writeln!(self.out, "{text}")?;
        self.out.flush()
    }

    /// 另起一行写一段暗色的说明：用量、重试、这一轮怎么结束的。
    fn note(&mut self, text: &str) -> io::Result<()> {
        match self.color {
            true => self.line(&format!("{DIM}{text}{RESET}")),
            false => self.line(text),
        }
    }

    /// 等人开口的提示，不换行。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn prompt(&mut self) -> io::Result<()> {
        self.fresh_line()?;
        write!(self.out, "你> ")?;
        self.out.flush()
    }

    /// 一条瞬时事件：一段增量，或者要重试了。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn transient(&mut self, transient: &Transient) -> io::Result<()> {
        match &transient.body {
            TransientBody::ModelDelta(delta) => self.piece(delta.index, &delta.piece),
            TransientBody::Status(status) => self.retry(status),
            TransientBody::ToolProgress(_) => Ok(()),
        }
    }

    /// 一段增量：块开始了、一段字、块收全了。
    fn piece(&mut self, index: usize, piece: &Piece) -> io::Result<()> {
        match piece {
            Piece::Start(kind) => {
                self.kinds.insert(index, kind.clone());
                self.fresh_line()?;
                match kind {
                    Kind::Reasoning if self.color => write!(self.out, "{DIM}")?,
                    Kind::Reasoning => write!(self.out, "思考：")?,
                    Kind::ToolCall { name } => {
                        write!(self.out, "（要调用 {name}，试玩台没有工具）")?
                    }
                    Kind::Text => {}
                }
            }
            Piece::Text(text) => {
                if !matches!(self.kinds.get(&index), Some(Kind::ToolCall { .. })) {
                    write!(self.out, "{text}")?;
                    self.fresh = text.ends_with('\n');
                }
            }
            Piece::End => {
                if matches!(self.kinds.get(&index), Some(Kind::Reasoning)) && self.color {
                    write!(self.out, "{RESET}")?;
                }
                self.fresh_line()?;
            }
        }
        self.out.flush()
    }

    /// 要重试了：等多久，第几次。出的什么错，前面 `model.called` 那一行写了。
    fn retry(&mut self, status: &Status) -> io::Result<()> {
        let retry = &status.retry;
        let seconds = retry.wait_ms as f64 / 1000.0;
        self.note(&format!(
            "（{seconds:.1} 秒后第 {}/{} 次重试）",
            retry.attempt, retry.limit
        ))
    }

    /// 一条落了盘的事件：用量、被打断的那一句、这一轮怎么结束的；回复本身已经边出边写过了。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn event(&mut self, event: &Event) -> io::Result<()> {
        match &event.body {
            Body::MessageAssistant(_) => {
                self.kinds.clear();
                self.fresh_line()
            }
            Body::ModelCalled(called) => self.called(called),
            Body::ContextInjected(fact) if fact.kind.as_str() == "reply_cut" => {
                self.note("（半截留下了，跟上一句被打断的提示再请求）")
            }
            Body::TurnEnded(ended) => match ended_text(&ended.reason) {
                Some(text) => self.note(&format!("（这一轮{text}）")),
                None => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// 一次请求记完了：用量那一行；没成的写出了什么错。
    fn called(&mut self, called: &ModelCalled) -> io::Result<()> {
        self.kinds.clear();
        if let Some(usage) = &called.usage {
            let input = usage.uncached + usage.cache_read + usage.cache_write;
            let mut text = format!(
                "（输入 {input}：命中 {}，没命中 {}",
                usage.cache_read, usage.uncached
            );
            if usage.cache_write > 0 {
                text.push_str(&format!("，写入缓存 {}", usage.cache_write));
            }
            text.push_str(&format!(" · 输出 {}", usage.output));
            if let Some(ms) = called.duration_ms {
                text.push_str(&format!(" · {:.1} 秒", ms as f64 / 1000.0));
            }
            if let Some(last) = self.last_input {
                let again = last.saturating_sub(usage.cache_read);
                text.push_str(&format!(" · 上一次发过的 {last} 里重算了 {again}"));
            }
            text.push('）');
            self.last_input = Some(input);
            self.note(&text)?;
        }
        match (&called.result, &called.error) {
            (CallResult::Ok, _) => Ok(()),
            (_, Some(error)) => self.note(&format!("（这次请求没成：{}）", error_text(error))),
            (result, None) => self.note(&format!("（这次请求没成：{}）", result.as_str())),
        }
    }

    /// 光标不在行首的，先换行。
    fn fresh_line(&mut self) -> io::Result<()> {
        if !self.fresh {
            writeln!(self.out)?;
            self.fresh = true;
        }
        Ok(())
    }
}

/// 这一轮怎么结束的，写给人看；正常走完的不写。
fn ended_text(reason: &EndReason) -> Option<String> {
    let text = match reason {
        EndReason::Completed => return None,
        EndReason::Interrupted => "被打断了",
        EndReason::Error => "出错结束了",
        EndReason::StepLimit => "走到了步数上限",
        EndReason::Aborted => "没走完就中止了",
        EndReason::Restarted => "被重启打断了",
        EndReason::Other(text) => text,
    };
    Some(text.to_string())
}

/// 出错的分类和原话，原话太长的截短。
fn error_text(error: &CallError) -> String {
    let class = match &error.class {
        ErrorClass::Retryable => "可以重试的错",
        ErrorClass::RateLimited => "限速",
        ErrorClass::ContextTooLong => "上下文超长",
        ErrorClass::Auth => "认证失败",
        ErrorClass::ContentPolicy => "内容策略",
        ErrorClass::Unclassified => "没分出类的错",
        ErrorClass::BadStream => "增量对不上",
        ErrorClass::EmptyReply => "回复是空的",
        ErrorClass::Other(text) => text,
    };
    let mut message: String = error.message.chars().take(MESSAGE_CHARS).collect();
    if message.len() < error.message.len() {
        message.push('…');
    }
    format!("{class}：{message}")
}
