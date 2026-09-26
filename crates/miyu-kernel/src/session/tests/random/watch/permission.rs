//! 看守查权限级别（`docs/designs/02-内核.md` 第六节「权限级别怎么切」）：照日志里切的，自己推出实际
//! 生效的那一级，收紧的当场换，放宽的等开回合、等请求。只读生效的时候不派写文件的调用，内核拦下
//! 的都是写文件的；回合中途注入的，排在这一步的全部工具结果后面；请求时，最近一块权限事实写的
//! 是现在的那一级，环境那一块写的是这一轮的工作目录。

use super::*;
use crate::event::{Level, Permission};

impl Watch {
    /// 这一批里的第 `k` 条，照权限的规矩记下、查。
    pub(super) fn permission_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        match &event.body {
            Body::PolicyChanged(changed) => {
                let new = changed
                    .permission
                    .clone()
                    .unwrap_or_else(|| panic!("种子 {seed}：切权限级别没写新的权限"));
                if rank(&new) < rank(&self.effective) {
                    self.effective = new.clone();
                }
                self.permission = new;
            }
            Body::TurnStarted(_) => self.effective = self.permission.clone(),
            Body::ContextInjected(fact)
                if event.by == By::Kernel && fact.kind.as_str() == "permission" =>
            {
                assert_eq!(
                    fact.text,
                    level_fact(&self.permission),
                    "种子 {seed}：权限那一块写的不是现在的那一级"
                );
                let opening = events[..k]
                    .iter()
                    .any(|event| matches!(event.body, Body::TurnStarted(_)));
                if !opening {
                    self.seen_paths.insert("切了级别以后注入");
                    let turn = self.open_turn();
                    assert!(
                        self.calls_in(turn)
                            .all(|call| self.resulted.contains(&call)),
                        "种子 {seed}：回合中途注入的，要排在这一步的全部工具结果后面"
                    );
                }
            }
            Body::ToolResult(result)
                if result.status == ToolStatus::Denied
                    && approval::text_of(event) == "read only" =>
            {
                let call_id = result.call_id;
                assert_eq!(event.by, By::Kernel, "种子 {seed}：拦下 {call_id} 的是内核");
                assert!(
                    self.writes(call_id),
                    "种子 {seed}：拦下的 {call_id} 不是要写入的"
                );
                assert_eq!(
                    rank(&self.effective),
                    0,
                    "种子 {seed}：只读没生效，却拦下了 {call_id}"
                );
                let tightened = events[..k]
                    .iter()
                    .any(|event| matches!(event.body, Body::PolicyChanged(_)));
                let replied = events[..k]
                    .iter()
                    .any(|event| matches!(event.body, Body::MessageAssistant(_)));
                self.seen_paths.insert(match (tightened, replied) {
                    (true, _) => "收紧时拦下还没派的",
                    (false, true) => "回复到了只读拦下",
                    (false, false) => "要写入的请求只读拦下",
                });
            }
            _ => {}
        }
    }

    /// 请求模型：放宽的这时生效；最近一块权限事实写的是现在的那一级，环境那一块写的是这一轮
    /// 的工作目录。
    pub(super) fn permission_request(&mut self) {
        let seed = self.seed;
        self.effective = self.permission.clone();
        assert_eq!(
            self.latest_fact("permission"),
            Some(level_fact(&self.permission)),
            "种子 {seed}：请求时，最近一块权限事实不是现在的那一级"
        );
        let cwd = &self.turn_cwd[&self.open_turn()];
        let env = self.latest_fact("env").unwrap_or_default();
        assert!(
            env.ends_with(&format!(r#" d="{cwd}"/>"#)),
            "种子 {seed}：请求时，环境那一块不是这一轮的工作目录 {cwd}：{env}"
        );
    }

    /// 派一个调用：只读生效的时候不派要写入的。
    pub(super) fn permission_run(&self, call_id: CallId, writes: bool) {
        assert!(
            !writes || !self.read_only_in_effect(),
            "种子 {}：只读生效的时候派了要写入的 {call_id}",
            self.seed
        );
    }

    /// 实际生效的是不是只读。
    pub(super) fn read_only_in_effect(&self) -> bool {
        rank(&self.effective) == 0
    }

    /// 这一步里有还没跑的写文件调用，只读也没生效：收紧当场拦下的窗口。
    pub(in super::super) fn write_waiting(&self) -> bool {
        if self.read_only_in_effect() || !self.turn_open() {
            return false;
        }
        self.calls_in(self.open_turn()).any(|call| {
            !self.running.contains(&call)
                && !self.resulted.contains(&call)
                && self.name_of(call) == "write"
        })
    }

    /// 内核最近注入的这一类事实的原文，撤掉的回合里的不算（08 C10）。
    fn latest_fact(&self, kind: &str) -> Option<String> {
        self.events
            .iter()
            .rev()
            .filter(|event| !self.undo.gone.contains(&event.seq))
            .find_map(|event| match &event.body {
                Body::ContextInjected(fact)
                    if event.by == By::Kernel && fact.kind.as_str() == kind =>
                {
                    Some(fact.text.clone())
                }
                _ => None,
            })
    }
}

/// 一份权限有多宽：只读 0、工作区 1、完全放开 2。
fn rank(permission: &Permission) -> u8 {
    match (permission.read_only, &permission.level) {
        (true, _) | (false, Level::Other(_)) => 0,
        (false, Level::Workspace) => 1,
        (false, Level::Full) => 2,
    }
}

/// 替身的模板写出来的权限那一块。
fn level_fact(permission: &Permission) -> String {
    let level = match rank(permission) {
        0 => "read_only",
        1 => "workspace",
        _ => "full",
    };
    format!(r#"<p l="{level}"/>"#)
}
