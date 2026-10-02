//! 测试用的口子（`net.md`「起草时定的」第 3 条）：本机的假服务器在回环上，地址闸本来不许去；测试也不该碰真的 DNS、
//! 不该看开发机上设的代理。只在 `testkit` 开关打开（和这个包自己的单元测试）时编进去，生产的构建里没有这段代码。

use std::net::{IpAddr, SocketAddr};

/// 测试时闸和代理照这个走，不照平时的。交给 [`crate::LinkPreview::testing`]。
#[derive(Debug, Clone, Default)]
pub struct Testing {
    /// 回环的地址（`127.0.0.0/8`、`::1`，嵌着回环 IPv4 的 IPv6 也算）当公网：本机的假服务器在回环上。别的段照旧。
    pub loopback_public: bool,
    /// 主机名只照这张表解析（名字，地址），不问系统：表里没有的当解析不出来。
    pub hosts: Vec<(String, IpAddr)>,
    /// 代理照这个，不读环境变量：`None` 是不走代理。
    pub proxy: Option<String>,
    /// `NO_PROXY` 照这个，写法和环境变量的一样。
    pub no_proxy: String,
}

impl Testing {
    /// 照表解析：名字不分大小写，表里没有的是空的。
    pub(crate) fn lookup(&self, name: &str, port: u16) -> Vec<SocketAddr> {
        self.hosts
            .iter()
            .filter(|(host, _)| host.eq_ignore_ascii_case(name))
            .map(|(_, ip)| SocketAddr::new(*ip, port))
            .collect()
    }
}
