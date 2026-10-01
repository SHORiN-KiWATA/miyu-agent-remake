//! 跟着这一句开的那一轮（施工 3-9 下）：`turn.started` 的 `cause` 是自己发的那条命令，就是它；之后照回合编号
//! 收它的推送。回答写标准输出，思考写标准错误（终端里灰色），用量加起来；结束了印用量、给退出码。
//!
//! 她做的每一步，结果来了印出来，也写标准错误（施工 4-5 下，[`super::steps`]）。思考、步骤、工作目录太宽那
//! 一句都是旁白：连着的几行步骤之间不空行，和回答之间空一行；一段思考、一块（执行命令、编辑标题下面有东西的）
//! 前后各空一行（施工 4-11，照 opencode）。
//!
//! 有几步因为要确认、这里没人能确认被拒的，数着：这一轮照常结束的，退出码 4，用量后面再印一行（施工 4-9）。
//!
//! `miyu redo` 也照这里跟着新的一轮（施工 4-7 再补）：回应到了先照 `miyu undo` 印撤掉了哪一轮（[`Follow::redoing`]）。核心写
//! 回应里给人看的几样要读日志，回应到的时候新的一轮可能已经开口、甚至说完了：在那以前推过来的先攒着，印完那几行再接着收。
//!
//! 她正忙时这一句不另开一轮，跟的是听到它的那一轮（施工 7-10，`joining.rs`）。开头那几行旁白（配置有错、项目配置没信任、
//! 沙盒用不了、目录太宽）在 `opening.rs`。
//!
//! `miyu ask` 还等子代理（施工 7-9，[`Follow::waits`]）：这一轮结束了，派出去的子代理还有没报的，接着跟被回报叫醒的几轮，
//! 都了结了才收尾（`waiting.rs`、`agents.rs`）；收尾的几行在 `ending.rs`。

mod agents;
mod compacting;
mod ending;
mod joining;
mod opening;
mod waiting;

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::steps::{self, Drawn, Steps};
use super::usage::Sum;
use super::{Format, Plan, Screen, exit};
use crate::shown::{GRAY, Line, RESET, say, write};
use crate::undo::{Direction, UndoPlan, redo_lines};

pub(crate) use ending::Leaving;

/// 回到行首、擦掉这一行：终端里原地刷新用（压缩的进度、等子代理的那一行）。
const REDRAW: &str = "\r\x1b[2K";

/// 收了一条以后怎么办。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// 接着收。
    Going,
    /// 都结束了：退出码。
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
    /// 屏幕上（两条通道合起来看）最后一行是空行，或者什么都还没印：一块前面不用再空（施工 4-11）。
    blank: bool,
    /// 刚印完一块、一段思考，后面欠一个空行：等下一样东西来了再写。
    owed: bool,
    /// 正在印一段思考：下一样不是思考的东西来了，这一段就完了。
    thinking: bool,
    /// 她调过的：结果来了照它写成一行。
    steps: Steps,
    /// 会话实际在哪个目录里干活：先当是头报的，核心的回应里说了就照它（施工 4-5 下）。
    cwd: String,
    /// 说过这里的项目配置还没信任了（施工 8-3）：一次 `miyu ask` 只说一次。
    untrusted: bool,
    usage: Sum,
    failure: Option<Failure>,
    /// 因为要确认、这里没人能确认被拒的有几步（施工 4-9）。
    unattended: u64,
    /// 正在压缩（施工 6-3 下）：哪一次摘要请求，终端里那一行进度画了没有。
    compacting: compacting::Compacting,
    /// 跟的是重做开的那一轮、回应还没到（施工 4-7 再补）：这时推过来的事件攒在这里，回应到了照撤销印那几行，再照先后收它们。
    redo: Option<Vec<Value>>,
    /// 等子代理（施工 7-9）：第一轮结束了还有没报的，接着跟被回报叫醒的几轮。只有 `miyu ask` 等。
    waits: bool,
    /// 第一轮结束过了：之后等着的时候开的每一轮都跟。
    ended_first: bool,
    /// 最近结束的那一轮为什么结束：收尾照它说（施工 7-9）。
    reason: String,
    /// 结束了的每一轮，照先后：`--format json` 的 `turns`（施工 7-9）。
    turns: Vec<Value>,
    /// 跟过的每一轮每次请求的用量加起来：最后那一行（施工 7-9）。`usage` 只是这一轮的。
    total: Sum,
    /// 等的子代理（这一次派出去的，施工 7-9；这一次留过言的，施工 7-9 补），还有几个没报。
    agents: agents::Agents,
    /// 等子代理的那一行画着没有（施工 7-9）。
    waiting: waiting::Waiting,
    /// 她正忙时跟住听到这一句的那一轮（施工 7-10）。
    joining: joining::Joining,
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
            blank: true,
            owed: false,
            thinking: false,
            steps: Steps::default(),
            cwd: plan.cwd.clone(),
            untrusted: false,
            usage: Sum::default(),
            failure: None,
            unattended: 0,
            compacting: compacting::Compacting::default(),
            redo: None,
            waits: false,
            ended_first: false,
            reason: String::new(),
            turns: Vec::new(),
            total: Sum::default(),
            agents: agents::Agents::default(),
            waiting: waiting::Waiting::default(),
            joining: joining::Joining::default(),
        }
    }

    /// 等子代理（施工 7-9，`docs/blueprint/cli/ask.md`「等子代理」）：`miyu ask` 才等，`miyu redo`、`miyu compact` 跟完一轮就走。
    pub(crate) fn waits(&mut self) {
        self.waits = true;
    }

    /// 跟的是 `session.redo` 开的那一轮（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：回应到了先印撤掉了哪一轮。
    pub(crate) fn redoing(&mut self) {
        self.redo = Some(Vec::new());
    }

    /// 重做的回应到了：会话在哪个目录里干活照回应的换上，不说目录太宽（重做不报敲命令时的目录）；照 `miyu undo` 印撤掉了
    /// 哪一轮，第一行接「，重新做」，不说怎么恢复，都是旁白。这时撤销、重发、新的一轮的开头都推过来了，模型还没开口。
    fn undone(&mut self, result: &Value, screen: &mut Screen<'_>) {
        if let Some(cwd) = result["cwd"].as_str() {
            self.cwd = cwd.to_string();
        }
        if self.plan.format != Format::Text {
            return;
        }
        let plan = UndoPlan {
            direction: Direction::Undo,
            session: None,
            language: self.plan.language,
            home: self.plan.home.clone(),
            color: screen.gray,
        };
        for line in redo_lines(result, &plan) {
            self.aside(&line, screen);
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
            if let Some(held) = self.redo.take() {
                self.undone(&message["result"], screen);
                for message in &held {
                    let step = self.take(message, screen);
                    if step != Step::Going {
                        return step;
                    }
                }
                return Step::Going;
            }
            self.opened(&message["result"], screen);
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
        // 重做的回应还没到：先攒着，撤掉了哪一轮那几行印在最前面。
        if let Some(held) = self.redo.as_mut() {
            held.push(message.clone());
            return Step::Going;
        }
        let event = &message["params"]["event"];
        let kind = event["kind"].as_str().unwrap_or_default();
        match kind {
            "turn.started" => {
                self.started(event, screen);
                return Step::Going;
            }
            // 回报不带回合编号（施工 7-9）：哪一轮里到的都认。
            "child.reported" => return self.child_reported(event, screen),
            _ => {}
        }
        // 她正忙时接上听到这一句的那一轮（施工 7-10，`joining.rs`）。
        self.join(event, screen);
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
            "tool.result" => {
                self.agents.result(body);
                self.result(event, screen);
            }
            "model.called" => {
                self.agents.called(body);
                self.joining.called(body);
                self.called(body, screen);
            }
            "compaction.progress" => self.compaction_progress(body, screen),
            "compaction.done" => self.compaction_done(body, screen),
            "context.compaction_paused" => self.compaction_paused(body, screen),
            "turn.ended" => return self.ended(body["reason"].as_str().unwrap_or_default(), screen),
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
                if !self.thinking {
                    self.paragraph(screen);
                }
                match screen.gray {
                    true => write(screen.err, &format!("{GRAY}{text}{RESET}")),
                    false => write(screen.err, text),
                }
                self.err_ends_line = text.ends_with('\n');
                self.aside = true;
                self.blank = false;
                self.thinking = true;
            }
            Some("text") => {
                self.part(screen, "\n");
                write(screen.out, text);
                self.blank = false;
                self.answered = true;
                self.answer_ends_line = text.ends_with('\n');
            }
            _ => {}
        }
    }

    /// 旁白印完了：还没换行的补上换行，再补 `gap`（回答前面空一行，收尾时不空）。一块后面欠的空行，就是回答前面的
    /// 这一行，不再多空。
    fn part(&mut self, screen: &mut Screen<'_>, gap: &str) {
        if !self.aside {
            return;
        }
        if !self.err_ends_line {
            write(screen.err, "\n");
        }
        if !gap.is_empty() {
            write(screen.err, gap);
            self.blank = true;
            self.owed = false;
            self.thinking = false;
        }
        self.err_ends_line = true;
        self.aside = false;
    }

    /// 另起一段（一段思考、一块）：还没换行的先换行；屏幕上最后一行不是空行、也不是什么都还没印的，空一行。欠着的
    /// 空行就是这一行。
    fn paragraph(&mut self, screen: &mut Screen<'_>) {
        if !self.err_ends_line {
            write(screen.err, "\n");
            self.err_ends_line = true;
        }
        if !self.blank {
            write(screen.err, "\n");
            self.blank = true;
        }
        self.owed = false;
    }

    /// 一段思考完了：后面欠一个空行。
    fn thought(&mut self) {
        if self.thinking {
            self.thinking = false;
            self.owed = true;
        }
    }

    /// 一块、一段思考后面欠着空行的，先写上。
    fn settle(&mut self, screen: &mut Screen<'_>) {
        if self.owed {
            if !self.err_ends_line {
                write(screen.err, "\n");
            }
            write(screen.err, "\n");
            self.err_ends_line = true;
            self.blank = true;
            self.owed = false;
        }
    }

    /// 一次结果（`tool.result` 整条事件）：印成旁白，`--format json` 不印。之后的回复和前面的隔开。因为要确认被拒的，
    /// 数上。
    fn result(&mut self, event: &Value, screen: &mut Screen<'_>) {
        let body = &event["body"];
        self.stepped = true;
        if steps::unattended(body) {
            self.unattended += 1;
        }
        if self.plan.format != Format::Text {
            return;
        }
        let by_tool = event["by"]["kind"] == json!("tool");
        match self.steps.result(body, by_tool, self.plan, &self.cwd) {
            Some(Drawn { title, below }) if below.is_empty() => self.aside(&title, screen),
            Some(drawn) => self.block(&drawn, screen),
            None => {}
        }
    }

    /// 印一块：前面空一行（屏幕上最后一行已经是空行、什么都还没印的不空），后面的空行欠着，等下一样东西来了再写。
    fn block(&mut self, drawn: &Drawn, screen: &mut Screen<'_>) {
        self.close_answer(screen);
        self.thinking = false;
        self.paragraph(screen);
        let mut text = drawn.title.paint(screen.gray);
        for line in &drawn.below {
            text.push_str(&line.paint(screen.gray));
        }
        write(screen.err, &text);
        self.err_ends_line = true;
        self.aside = true;
        self.blank = false;
        self.owed = true;
    }

    /// 回答那一行还没完的（她先说了「我先看看」再调工具），先在标准输出上换行：旁白不接在回答后面，重定向进文件的，
    /// 前后两段回答也隔开了。
    fn close_answer(&mut self, screen: &mut Screen<'_>) {
        if self.answered && !self.answer_ends_line {
            write(screen.out, "\n");
            self.answer_ends_line = true;
        }
    }

    /// 印一行旁白。回答那一行还没完的，先在标准输出上换行（[`Follow::close_answer`]）；思考那一行还没完的，也先
    /// 换行；一块、一段思考后面欠着空行的，先空一行。和回答之间的空行，等回答来了再写。
    fn aside(&mut self, line: &Line, screen: &mut Screen<'_>) {
        self.close_answer(screen);
        self.thought();
        self.settle(screen);
        if !self.err_ends_line {
            write(screen.err, "\n");
        }
        write(screen.err, &line.paint(screen.gray));
        self.err_ends_line = true;
        self.aside = true;
        self.blank = false;
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

    /// 一次请求的记录：用量加起来；出错的记下，后来又成了的（重试）当没出过。压缩的摘要请求出错的，说压缩失败
    /// （施工 6-3 下）。
    fn called(&mut self, body: &Value, screen: &mut Screen<'_>) {
        self.compaction_called(body, screen);
        self.usage.add(&body["usage"]);
        self.total.add(&body["usage"]);
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
}

#[cfg(test)]
mod tests;
