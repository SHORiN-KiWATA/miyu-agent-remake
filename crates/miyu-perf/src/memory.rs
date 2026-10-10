//! 进程占多少内存（`23-性能预算.md` 第一节：按进程分开报，PSS 和私有匿名内存）。
//!
//! 读 `/proc/<进程号>/smaps_rollup`，子进程照 `/proc/<进程号>/task/<线程号>/children` 找：只有 Linux 上读得到。
//! macOS、Windows 上怎么量是 23「后续再定」的一条，那里读不到，交回空的，表里写「不量」。

use std::path::Path;

use serde_json::{Value, json};

/// 一个进程的内存，单位 KB，照 `smaps_rollup` 的写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// 常驻：`Rss`。
    pub rss: u64,
    /// 按份分摊的常驻：`Pss`。几个进程共享的页各算一份，加起来是整台机器真花的。
    pub pss: u64,
    /// 匿名内存：`Anonymous`。堆、栈这些，不是从文件映射来的，就是 23 说的私有匿名内存。
    pub anon: u64,
}

/// 一个进程：进程号、名字、内存。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    /// 进程号。
    pub pid: u32,
    /// `comm` 里的名字。
    pub name: String,
    /// 内存。
    pub usage: Usage,
}

impl Process {
    /// 写进原始数据的样子。
    pub fn to_json(&self) -> Value {
        json!({"pid": self.pid, "name": self.name, "rss_kb": self.usage.rss,
            "pss_kb": self.usage.pss, "anon_kb": self.usage.anon})
    }
}

/// 读 `smaps_rollup` 的字：三项都有才算读成。
pub fn parse_rollup(text: &str) -> Option<Usage> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(name)?.strip_prefix(':')?;
            rest.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()
        })
    };
    Some(Usage {
        rss: field("Rss")?,
        pss: field("Pss")?,
        anon: field("Anonymous")?,
    })
}

/// 读 `children` 的字：空格隔开的进程号。
pub fn parse_children(text: &str) -> Vec<u32> {
    text.split_whitespace()
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

/// `pid` 和它的子孙进程，各自的内存，`pid` 排第一。量不了的平台、进程已经没了的是空的。
pub fn tree(pid: u32) -> Vec<Process> {
    let mut found = Vec::new();
    let mut pending = vec![pid];
    while let Some(next) = pending.pop() {
        if let Some(process) = one(next) {
            found.push(process);
            pending.extend(children(next));
        }
    }
    found
}

/// 一个进程的内存。没有 `/proc` 的平台读不到，交回 `None`。
fn one(pid: u32) -> Option<Process> {
    let proc = Path::new("/proc").join(pid.to_string());
    let usage = parse_rollup(&std::fs::read_to_string(proc.join("smaps_rollup")).ok()?)?;
    let name = std::fs::read_to_string(proc.join("comm")).ok()?;
    Some(Process {
        pid,
        name: name.trim().to_string(),
        usage,
    })
}

/// 子进程：每个线程拉起的都算。
fn children(pid: u32) -> Vec<u32> {
    let tasks = Path::new("/proc").join(pid.to_string()).join("task");
    let Ok(entries) = std::fs::read_dir(tasks) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("children")).ok())
        .flat_map(|text| parse_children(&text))
        .collect()
}

#[cfg(test)]
mod tests;
