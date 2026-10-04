//! 换模型、冷却（蓝图 `tui.md`「配置与模型」第 2、7 条，核心 8-9）：出错换到别的端点写一行暗色的「↻ 换了模型」和为什么；
//! 都在冷却时记着，底栏模型那一格变黄，又有一次请求成了变回原样。

use jiff::Timestamp;

use super::{Kind, Transcript};
use crate::config::Texts;
use crate::core::{CallError, Current, Limits};

/// 换模型、冷却要记着的。
#[derive(Debug, Default)]
pub struct ModelWatch {
    /// 最近一次出错的分类：换端点那一行写为什么（成了也不清，换端点的推送在成了以后才来）。
    last_error: Option<String>,
    /// 最近一次出错的请求试的端点/模型：换端点那一行的「原来的」（换端点的推送在答完以后才来，那时底栏已经换了）。
    tried: Option<String>,
    /// 会话现在用的引用（模型、`@池`、挡位）：`/model` 照它标「当前」（核心 8-10）。
    reference: Option<String>,
    /// 都在冷却：最早什么时候恢复，还不知道的是 `None`；没在冷却是外面那层 `None`。
    cooling: Option<Option<Timestamp>>,
    /// 接下来那个模型真用的思考强度：底栏写在模型名后面（核心 8-18，`/effort`）。
    effort: Option<String>,
}

impl Transcript {
    /// 出错的那次请求试的是哪个端点、模型。
    pub(super) fn tried(&mut self, endpoint: &str, model: &str) {
        self.models.tried = Some(format!("{endpoint}/{model}"));
    }

    /// 一次请求出错了。
    pub(super) fn call_failed(&mut self, error: CallError) {
        self.models.last_error = Some(error.class.clone());
        if error.class == "cooling" {
            self.models.cooling = Some(None);
        }
        self.failure = Some(error);
    }

    /// 一次请求成了：前面记着的错、冷却都不算了。
    pub(super) fn call_ok(&mut self) {
        self.failure = None;
        self.models.cooling = None;
    }

    /// 换了模型：底栏的模型、限额当场换；出错换的写一行暗色的「↻ 换了模型：原来的 → 换成的」，下一行写为什么。
    #[allow(clippy::too_many_arguments)]
    pub(super) fn model_changed(
        &mut self,
        endpoint: Option<String>,
        model: Option<String>,
        limits: Option<Limits>,
        failover: bool,
        reference: Option<String>,
        effort: Option<String>,
        texts: &Texts,
    ) {
        if reference.is_some() {
            self.models.reference = reference;
        }
        self.models.effort = effort;
        let current = self.model.as_ref().map(|(m, e)| format!("{e}/{m}"));
        let from = self.models.tried.take().or(current);
        if let (Some(model), Some(endpoint)) = (model, endpoint) {
            self.model = Some((model, endpoint));
        }
        if let Some(limits) = limits {
            self.limits = limits;
        }
        self.models.cooling = None;
        if !failover {
            return;
        }
        let to = self.model.as_ref().map(|(m, e)| format!("{e}/{m}"));
        let line = texts
            .models
            .failover
            .replace("{from}", from.as_deref().unwrap_or("?"))
            .replace("{to}", to.as_deref().unwrap_or("?"));
        self.note(Kind::Note, line);
        if let Some(class) = self.models.last_error.take() {
            let why = super::failure::class_name(&class, texts);
            self.note(Kind::Note, why);
        }
    }

    /// 都在冷却：最早什么时候恢复（`None` 是还不知道）；没在冷却的是 `None`。
    pub fn cooling(&self) -> Option<Option<Timestamp>> {
        self.models.cooling
    }

    /// 核心交回了冷却着的模型里最早恢复的时刻（`model.list`）。没在冷却的不记。
    pub fn cooling_until(&mut self, until: Option<Timestamp>) {
        if self.models.cooling.is_some() {
            self.models.cooling = Some(until);
        }
    }

    /// 会话现在用的引用。
    pub fn model_ref(&self) -> Option<&str> {
        self.models.reference.as_deref()
    }

    /// 还没开会话：底栏照默认的聊天模型（`models.chat`）写模型和这一档；开了会话的不动（照推来的）。
    pub fn show_default(&mut self, reference: &str, effort: Option<String>) {
        if self.models.reference.is_some() {
            return;
        }
        if let Some((endpoint, model)) = reference.split_once('/') {
            self.model = Some((model.to_string(), endpoint.to_string()));
        }
        self.models.effort = effort;
    }

    /// 底栏现在写的模型、思考强度：开新会话时带过去，等核心交回默认的再换（不闪一下空的）。
    pub fn footer_model(&self) -> (Option<(String, String)>, Option<String>) {
        (self.model.clone(), self.models.effort.clone())
    }

    /// 接着写带过来的模型、思考强度（[`Transcript::footer_model`]）。
    pub fn keep_footer_model(
        &mut self,
        (model, effort): (Option<(String, String)>, Option<String>),
    ) {
        self.model = model;
        self.models.effort = effort;
    }

    /// 换了思考强度（`/effort` 写完再要的 `model.list`）：底栏当场照新的最终值写。
    pub fn set_effort(&mut self, effort: Option<String>) {
        self.models.effort = effort;
    }

    /// 接下来那个模型真用的思考强度；什么都不发的是 `None`（底栏，`/effort`）。
    pub fn effort(&self) -> Option<&str> {
        self.models.effort.as_deref()
    }

    /// 订阅回应里会话现在用的模型：底栏照它画，记下引用。
    pub(super) fn current_model(&mut self, current: Current) {
        if let (Some(model), Some(endpoint)) = (current.model, current.endpoint) {
            self.model = Some((model, endpoint));
        }
        self.models.reference = Some(current.reference);
        self.models.effort = current.effort;
    }

    /// `/model` 换成了（下一个回合开始生效）：记下引用，底栏当场写成选的那个（`供应商/模型` 的分开写，池、挡位照写）。
    pub(super) fn configured(&mut self, reference: String) {
        let shown = match reference.split_once('/') {
            Some((endpoint, model)) if !reference.starts_with('@') => {
                (model.to_string(), endpoint.to_string())
            }
            _ => (reference.clone(), String::new()),
        };
        self.model = Some(shown);
        self.models.reference = Some(reference);
        // 原来那个模型的思考强度不是这个的：先不写，等 `model.list`、推来的（池的照推来的）。
        self.models.effort = None;
    }

    /// 会话换了模型（`session.policy_changed`）：记下引用；钉着的没了、内核退回默认的写一行暗色的（第 8 条）。
    pub(super) fn model_set(&mut self, reference: String, replaced: Option<String>, texts: &Texts) {
        if let Some(replaced) = replaced {
            let line = texts
                .models
                .replaced
                .replace("{from}", &replaced)
                .replace("{to}", &reference);
            self.note(Kind::Note, line);
        }
        self.models.reference = Some(reference);
    }
}
