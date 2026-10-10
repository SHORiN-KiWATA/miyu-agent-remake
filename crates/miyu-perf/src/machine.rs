//! 量的条件（`23-性能预算.md` 第一节「写清条件」）：机器、系统、构建、提交。
//!
//! 不写主机名、用户名、路径：结果要进公开的仓库。读不出来的一格是 `None`，表里写「—」。

use std::process::Command;

use serde_json::{Value, json};

/// 量的条件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    /// CPU 型号。
    pub cpu: Option<String>,
    /// 逻辑核数。
    pub cores: Option<usize>,
    /// 内存，字节。
    pub memory: Option<u64>,
    /// `linux`、`macos`、`windows`。
    pub os: String,
    /// `x86_64`、`aarch64`。
    pub arch: String,
    /// 内核版本。
    pub kernel: Option<String>,
    /// `rustc -V`。
    pub rustc: Option<String>,
    /// 提交的短哈希；工作区有没提交的改动的，后面跟「+改动」。
    pub commit: Option<String>,
}

impl Machine {
    /// 读一次。
    pub fn read() -> Machine {
        Machine {
            cpu: cpu(),
            cores: std::thread::available_parallelism().ok().map(usize::from),
            memory: memory(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            kernel: kernel(),
            rustc: output("rustc", &["-V"]),
            commit: commit(),
        }
    }

    /// 写进原始数据的样子。
    pub fn to_json(&self) -> Value {
        json!({"cpu": self.cpu, "cores": self.cores, "memory_bytes": self.memory, "os": self.os,
            "arch": self.arch, "kernel": self.kernel, "rustc": self.rustc, "commit": self.commit})
    }
}

/// 跑一个命令，交回标准输出去掉首尾空白；跑不成、没输出的是 `None`。
fn output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && !text.is_empty()).then_some(text)
}

/// 提交：短哈希，有没提交的改动的标出来。
fn commit() -> Option<String> {
    let hash = output("git", &["rev-parse", "--short", "HEAD"])?;
    let dirty = output("git", &["status", "--porcelain"]).is_some();
    Some(if dirty {
        format!("{hash}+改动")
    } else {
        hash
    })
}

/// `/proc/cpuinfo` 的第一个 `model name`。
pub fn parse_cpuinfo(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == "model name").then(|| value.trim().to_string())
    })
}

/// `/proc/meminfo` 的 `MemTotal`，换成字节。
pub fn parse_meminfo(text: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix("MemTotal:")?;
        let kb: u64 = rest.trim().strip_suffix("kB")?.trim().parse().ok()?;
        Some(kb * 1024)
    })
}

/// CPU 型号：Linux 读 `/proc/cpuinfo`，macOS 问 `sysctl`。
fn cpu() -> Option<String> {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| parse_cpuinfo(&text))
        .or_else(|| output("sysctl", &["-n", "machdep.cpu.brand_string"]))
}

/// 内存：Linux 读 `/proc/meminfo`，macOS 问 `sysctl`。
fn memory() -> Option<u64> {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| parse_meminfo(&text))
        .or_else(|| output("sysctl", &["-n", "hw.memsize"])?.parse().ok())
}

/// 内核版本：Linux 读 `/proc`，macOS 问 `uname`。
fn kernel() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|text| text.trim().to_string())
        .or_else(|| output("uname", &["-r"]))
}

#[cfg(test)]
mod tests;
