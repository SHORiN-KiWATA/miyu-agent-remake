//! `ask_user`（`docs/blueprint/tools/ask_user.md`，施工 D-2）：她干活中途问人一组题、等回答，回答成了这次调用的结果。
//! 参数照 Claude Code 的 AskUserQuestion，题数、选项数都不设上限（`10-自带软件.md` 第三节）。访问类别是读：只问人，
//! 什么都不改。经 [`Call::questions`] 交给执行器：执行器交给内核记 `question.asked`，人用 `session.answer` 回答，回答落了盘
//! 再送回来。
//!
//! 结果一道一行：`"<问的话>" = <回答>`，选了的照标题、逗号隔开，自己写的接在后面，备注写在括号里，没答的写没答；最后一行
//! 说照这些回答接着做。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::event::{Choice, Question, Response};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{ASK_USER, Call, Done, Progress, Running, Spec, Tool};

use crate::common::{Common, said};
use crate::load::{self, LoadError, say};

/// `ask_user`。
pub(crate) struct AskUser {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/ask_user/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    answer: Template,
    no_answer: Template,
    note: Template,
    end: Template,
    unattended: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    questions: Vec<QuestionArg>,
}

/// 一道题，字段名照 Claude Code。
#[derive(Deserialize)]
struct QuestionArg {
    question: String,
    #[serde(default)]
    header: Option<String>,
    #[serde(default)]
    options: Vec<OptionArg>,
    #[serde(default, rename = "multiSelect")]
    multi_select: Option<bool>,
}

/// 一个选项。
#[derive(Deserialize)]
struct OptionArg {
    label: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    preview: Option<String>,
}

impl AskUser {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<AskUser, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, ASK_USER, name, fields);
        Ok(AskUser {
            spec: load::spec(resources, ASK_USER, Access::Read)?,
            texts: Texts {
                common,
                answer: text("answer", &["question", "answer"])?,
                no_answer: text("no-answer", &[])?,
                note: text("note", &["notes"])?,
                end: text("end", &[])?,
                unattended: text("unattended", &[])?,
            },
        })
    }
}

impl Tool for AskUser {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            if args.questions.is_empty() {
                return texts.common.bad_args(&"`questions` is empty");
            }
            // 没有端口的：这里没人能回答（工具面上本来就不给，兜底）。
            let Some(port) = call.questions.clone() else {
                return Done::error(say(&texts.unattended, &[])).said(said("ask_user/unattended"));
            };
            let questions: Vec<Question> = args.questions.into_iter().map(question).collect();
            // 没答到就了结了：结果由内核照原来的规矩补，这里交回的不再记。
            let Some(answers) = port.ask(questions.clone()).await else {
                return Done::stopped();
            };
            Done::ok(texts.answered(&questions, &answers)).said(said("ask_user/answered"))
        })
    }
}

/// 参数里的一道题换成内核记的样子：`multiSelect` 记成 `multiple`。
fn question(arg: QuestionArg) -> Question {
    Question {
        header: arg.header,
        question: arg.question,
        options: arg
            .options
            .into_iter()
            .map(|option| Choice {
                label: option.label,
                description: option.description,
                preview: option.preview,
            })
            .collect(),
        multiple: arg.multi_select.unwrap_or(false),
    }
}

impl Texts {
    /// 一道一行，最后一行说照这些回答接着做。回答比题少的（不该有，内核查过对得上）当没答。
    fn answered(&self, questions: &[Question], answers: &[Response]) -> String {
        let mut text = String::new();
        for (k, question) in questions.iter().enumerate() {
            let answer = answers.get(k).map_or_else(
                || say(&self.no_answer, &[]),
                |response| self.response(response),
            );
            text.push_str(&say(
                &self.answer,
                &[("question", &question.question), ("answer", &answer)],
            ));
        }
        text.push_str(&say(&self.end, &[]));
        text
    }

    /// 一道的回答：选了的照标题、自己写的，逗号隔开；都没有的是没答；有备注的接在后面。
    fn response(&self, response: &Response) -> String {
        let mut parts: Vec<&str> = response.picked.iter().map(String::as_str).collect();
        if let Some(text) = response.text.as_deref() {
            parts.push(text);
        }
        let mut answer = match parts.is_empty() {
            true => say(&self.no_answer, &[]),
            false => parts.join(", "),
        };
        if let Some(notes) = response.notes.as_deref() {
            answer.push_str(&say(&self.note, &[("notes", notes)]));
        }
        answer
    }
}
