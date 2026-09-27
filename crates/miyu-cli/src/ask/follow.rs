//! 跟着这一句开的那一轮（施工 3-9 下）：`turn.started` 的 `cause` 是自己发的那条命令，就是它；之后照回合编号
//! 收它的推送。回答写标准输出，思考写标准错误（终端里灰色），用量加起来；结束了印用量、给退出码。
//!
//! 她做的每一步，结果来了印成一行，也写标准错误（施工 4-5 下，[`super::steps`]）。思考、步骤、工作目录太宽那
//! 一句都是旁白：旁白之间不空行，和回答之间空一行。

use std::collections::BTreeMap;
use std::io::Write;

use serde_json::{Value, json};

use super::steps::{self, GRAY, Line, RESET, Steps};
use super::usage::Sum;
use super::{Format, Plan, Screen, exit};

/// 收了一条以后怎么办。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// 接着收。
    Going,
    /// 这一轮结束了：退出码。
    Done(u8),
    /// 掉队了：重新订阅。
    Resubscribe,
}

/// 这一轮出错时 `model.called` 里说的。
#[derive(Debug, Clone)]
struct Failure {
    /// 分类。
    class: String,
    /// 原话。
    message: String,
    /// 发出去了：没发出去就失败了的，没有端点。
    sent: bool,
}

/// 跟着一轮。
pub(crate) struct Follow<'p> {
    session: String,
    /// 自己发的那条命令的编号。
    sent: String,
    plan: &'p Plan,
    /// 这一轮的编号：认出来以前没有。
    turn: Option<u64>,
    /// 每一块是什么：`text`、`reasoning`、`tool_call`。
    kinds: BTreeMap<u64, String>,
    /// 回答的全文，照最后写成的回复：给 `--format json`。
    answer: String,
    /// 上一段回复以后她做过步骤：下一段回复的正文和前面的隔开。
    stepped: bool,
    /// 标准输出上写过回答；最后一个字是不是换行。
    answered: bool,
    answer_ends_line: bool,
    /// 标准错误上最后一个字是不是换行。
    err_ends_line: bool,
    /// 印过旁白（思考、步骤……），还没和回答隔开。
    aside: bool,
    /// 她调过的：结果来了照它写成一行。
    steps: Steps,
    /// 会话实际在哪个目录里干活：先当是头报的，核心的回应里说了就照它（施工 4-5 下）。
    cwd: String,
    usage: Sum,
    failure: Option<Failure>,
}

impl<'p> Follow<'p> {
    /// 跟着会话 `session` 里命令 `sent` 开的那一轮。
    pub(crate) fn new(session: &str, sent: &str, plan: &'p Plan) -> Follow<'p> {
        Follow {
            session: session.to_string(),
            sent: sent.to_string(),
            plan,
            turn: None,
            kinds: BTreeMap::new(),
            answer: String::new(),
            stepped: false,
            answered: false,
            answer_ends_line: true,
            err_ends_line: true,
            aside: false,
            steps: Steps::default(),
            cwd: plan.cwd.clone(),
            usage: Sum::default(),
            failure: None,
        }
    }

    /// 核心说会话实际在 `used` 里干活：和头报的不一样，就是目录太宽、退回了账号的工作区，说一句。造会话、
    /// 说话的回应都带着它，一样的不再说：一次 `miyu ask` 只说一次。
    pub(crate) fn moved(&mut self, used: &str, screen: &mut Screen<'_>) {
        if used == self.cwd {
            return;
        }
        self.cwd = used.to_string();
        if self.plan.format == Format::Text {
            self.aside(&steps::moved(self.plan, used), screen);
        }
    }

    /// 收一条回应或推送。
    pub(crate) fn take(&mut self, message: &Value, screen: &mut Screen<'_>) -> Step {
        // 自己发的那条命令的回应：被拒绝的（没有这个会话、会话停了……）说清楚就走；接受了的，说会话实际在
        // 哪个目录里干活。
        if message["id"] == json!(self.sent) {
            if let Some(error) = message.get("error") {
                let reason = error["message"].as_str().unwrap_or_default();
                say(screen.err, &self.plan.language.refused(reason));
                return Step::Done(exit::ERROR);
            }
            if let Some(used) = message["result"]["cwd"].as_str() {
                self.moved(used, screen);
            }
            return Step::Going;
        }
        if message["params"]["session"] != json!(self.session) {
            return Step::Going;
        }
        match message["method"].as_str() {
            Some("resync") => return Step::Resubscribe,
            Some("event") => {}
            _ => return Step::Going,
        }
        let event = &message["params"]["event"];
        let kind = event["kind"].as_str().unwrap_or_default();
        if kind == "turn.started" && event["cause"] == json!(self.sent) {
            self.turn = event["turn"].as_u64();
            return Step::Going;
        }
        if self.turn.is_none() || event["turn"].as_u64() != self.turn {
            return Step::Going;
        }
        let body = &event["body"];
        match kind {
            "model.delta" => self.delta(body, screen),
            "message.assistant" => {
                self.reply(body);
                self.steps.reply(body);
            }
            "tool.result" => self.result(body, screen),
            "model.called" => self.called(body),
            "turn.ended" => {
                return Step::Done(self.end(body["reason"].as_str().unwrap_or_default(), screen));
            }
            _ => {}
        }
        Step::Going
    }

    /// 一段增量：块开始时记下它是什么；回答、思考边收边打，工具调用不打。块的「收全」不看：驱动等流完了才把
    /// 几块一起收（`samples/drivers/openai-chat/streams/deepseek-reasoning-tools.txt`），回答开始时思考那一块
    /// 还没收。思考和回答之间的空行在回答的第一段前面写。
    fn delta(&mut self, body: &Value, screen: &mut Screen<'_>) {
        let index = body["index"].as_u64().unwrap_or(0);
        if let Some(start) = body["start"].as_str() {
            self.kinds.insert(index, start.to_string());
            return;
        }
        let kind = self.kinds.get(&index).map(String::as_str);
        let Some(text) = body["text"].as_str().filter(|text| !text.is_empty()) else {
            return;
        };
        if self.plan.format != Format::Text {
            return;
        }
        match kind {
            Some("reasoning") => {
                match screen.gray {
                    true => write(screen.err, &format!("{GRAY}{text}{RESET}")),
                    false => write(screen.err, text),
                }
                self.err_ends_line = text.ends_with('\n');
                self.aside = true;
            }
            Some("text") => {
                self.part(screen, "\n");
                write(screen.out, text);
                self.answered = true;
                self.answer_ends_line = text.ends_with('\n');
            }
            _ => {}
        }
    }

    /// 旁白印完了：还没换行的补上换行，再补 `gap`（回答前面空一行，收尾时不空）。
    fn part(&mut self, screen: &mut Screen<'_>, gap: &str) {
        if !self.aside {
            return;
        }
        if !self.err_ends_line {
            write(screen.err, "\n");
        }
        write(screen.err, gap);
        self.err_ends_line = true;
        self.aside = false;
    }

    /// 一次结果：印成一行旁白，`--format json` 不印。之后的回复和前面的隔开。
    fn result(&mut self, body: &Value, screen: &mut Screen<'_>) {
        self.stepped = true;
        if self.plan.format != Format::Text {
            return;
        }
        if let Some(line) = self.steps.result(body, self.plan, &self.cwd) {
            self.aside(&line, screen);
        }
    }

    /// 印一行旁白。回答那一行还没完的（她先说了「我先看看」再调工具），先在标准输出上换行：旁白不接在回答
    /// 后面，重定向进文件的，前后两段回答也隔开了。思考那一行还没完的，也先换行。和回答之间的空行，等回答
    /// 来了再写。
    fn aside(&mut self, line: &Line, screen: &mut Screen<'_>) {
        if self.answered && !self.answer_ends_line {
            write(screen.out, "\n");
            self.answer_ends_line = true;
        }
        if !self.err_ends_line {
            write(screen.err, "\n");
        }
        write(screen.err, &line.paint(screen.gray));
        self.err_ends_line = true;
        self.aside = true;
    }

    /// 最后写成的回复：正文块给 `--format json`。和前一段回复之间隔着步骤、前一段又没换行的，补一个换行，
    /// 和标准输出上的一样。
    fn reply(&mut self, body: &Value) {
        let mut text = String::new();
        for block in body["blocks"].as_array().into_iter().flatten() {
            if block["type"] == json!("text")
                && let Some(piece) = block["text"].as_str()
            {
                text.push_str(piece);
            }
        }
        if text.is_empty() {
            return;
        }
        if self.stepped && !self.answer.is_empty() && !self.answer.ends_with('\n') {
            self.answer.push('\n');
        }
        self.stepped = false;
        self.answer.push_str(&text);
    }

    /// 一次请求的记录：用量加起来；出错的记下，后来又成了的（重试）当没出过。
    fn called(&mut self, body: &Value) {
        self.usage.add(&body["usage"]);
        self.failure = match body["result"].as_str() {
            Some("error") => Some(Failure {
                class: body["error"]["class"]
                    .as_str()
                    .unwrap_or("other")
                    .to_string(),
                message: body["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                sent: !body["endpoint"].is_null(),
            }),
            _ => None,
        };
    }

    /// 这一轮结束了：补上回答末尾的换行，印用量，说为什么结束，交回退出码。
    fn end(&mut self, reason: &str, screen: &mut Screen<'_>) -> u8 {
        let language = &self.plan.language;
        let (code, note) = match (reason, &self.failure) {
            ("completed", _) => (exit::OK, None),
            ("interrupted", _) => (exit::INTERRUPTED, Some(language.interrupted())),
            ("error", Some(failure)) if failure.class == "auth" && !failure.sent => {
                (exit::NO_MODEL, Some(language.no_model()))
            }
            ("error", Some(failure)) => (
                exit::ERROR,
                Some(language.failed(&failure.class, &failure.message)),
            ),
            ("error", None) => (exit::ERROR, Some(language.failed("other", ""))),
            (other, _) => (exit::ERROR, Some(language.unfinished(other))),
        };
        match self.plan.format {
            Format::Text => {
                // 只想了、没回答（例如被打断了）：思考那一行收个尾。
                self.part(screen, "");
                if self.answered && !self.answer_ends_line {
                    write(screen.out, "\n");
                }
                if self.usage.seen {
                    let line = language.usage(&self.usage);
                    match screen.gray {
                        true => write(screen.err, &format!("{GRAY}{line}{RESET}\n")),
                        false => write(screen.err, &format!("{line}\n")),
                    }
                }
            }
            Format::Json => {
                let mut result = json!({
                    "session": self.session,
                    "turns": [{"text": self.answer, "usage": self.usage.json()}],
                });
                if let Some(failure) = &self.failure {
                    result["error"] = json!({"class": failure.class, "message": failure.message});
                }
                write(screen.out, &format!("{result}\n"));
            }
        }
        if let Some(note) = note {
            say(screen.err, &note);
        }
        code
    }
}

/// 写一段，马上送出去：边收边打。写不出去的不管（例如标准错误被关了），不影响别的。
#[expect(clippy::let_underscore_must_use, reason = "写不出去也没有别处可说")]
pub(crate) fn write(to: &mut dyn Write, text: &str) {
    let _ = to.write_all(text.as_bytes()).and_then(|()| to.flush());
}

/// 说一句话，带换行。
pub(crate) fn say(to: &mut dyn Write, line: &str) {
    write(to, &format!("{line}\n"));
}

#[cfg(test)]
mod tests;
