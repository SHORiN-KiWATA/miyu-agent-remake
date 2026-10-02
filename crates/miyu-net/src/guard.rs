//! 地址闸（`net.md`「怎么走」第 3 条，照桥的 `guard.rs`，桥照的是旧版 `tools/net_guard.rs`）：网址是模型写的、
//! 别人发来的，核心照它去连，这里挡住内网、本机。三层依次过，每一跳都过：
//!
//! 1. 样子：只认 http、https；不带用户名、密码；`localhost`、`*.localhost`、`*.local` 不去；主机写的就是 IP 的
//!    照第 3 层判（`127.1`、`0x7f000001`、`2130706433` 这些写法读网址时已经规整成 `127.0.0.1`）。
//! 2. 解析：解析出来的每一个地址都要是公网的，有一个不是就整个不去。解析好的地址交回去，由 `fetch.rs` 钉进这一跳
//!    （不走代理的）：查过的就是连上的，中间没有第二次解析（防 DNS rebinding）。
//! 3. IP 段：[`is_public`]。
//!
//! 走不走代理不在这里判（`proxy.rs`）：这里只交回解析的结果，解析不出来的怎么办由 `fetch.rs` 照走不走代理定
//! （第 5 条）。测试的口子（`testkit` 开关）只在这里生效：回环当公网、只照一张表解析。

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use reqwest::Url;

/// 这一跳过了闸以后，主机在哪。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resolved {
    /// 主机写的就是 IP（已经过了第 3 层）：照它连，不用解析、不用钉。
    Literal,
    /// 本机解析出来的地址，都是公网的：不走代理的钉进这一跳。
    Addresses(Vec<SocketAddr>),
    /// 本机解析不出来（出错、超时、一个都没有）：走代理的照样交给代理，不走的连不上（第 5 条）。
    Nowhere,
}

/// 过不了闸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refused;

/// 地址闸：平时就是三层；`testkit` 开关打开时多一个测试的口子（`testkit.rs` 的 `Testing`）。
#[derive(Debug, Clone, Default)]
pub(crate) struct Guard {
    #[cfg(any(test, feature = "testkit"))]
    testing: Option<crate::testkit::Testing>,
}

impl Guard {
    /// 平时的闸：没有任何口子。
    pub(crate) fn new() -> Guard {
        Guard::default()
    }

    /// 测试的闸：回环当不当公网、照哪张表解析，照 `testing`。
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn testing(testing: crate::testkit::Testing) -> Guard {
        Guard {
            testing: Some(testing),
        }
    }

    /// 这个地址能不能去：公网的能去（第 3 层）。测试打开了「回环当公网」的，回环也能去。
    pub(crate) fn allows(&self, ip: IpAddr) -> bool {
        #[cfg(any(test, feature = "testkit"))]
        if self
            .testing
            .as_ref()
            .is_some_and(|testing| testing.loopback_public)
            && is_loopback(ip)
        {
            return true;
        }
        is_public(ip)
    }

    /// 第 1 层（主机写的就是 IP 的连第 3 层一起判），不解析。
    pub(crate) fn shape(&self, url: &Url) -> bool {
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };
        if let Some(ip) = literal(host) {
            return self.allows(ip);
        }
        let name = host.trim_end_matches('.').to_ascii_lowercase();
        !(name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local"))
    }

    /// 过闸：样子不合规的、解析出不是公网的地址的 [`Refused`]；过了的交回主机在哪。解析最多等 `budget`。
    ///
    /// # Errors
    ///
    /// 过不了闸（[`Refused`]）：样子不合规、没有端口、解析出来的地址有一个不是公网的。
    pub(crate) async fn pass(&self, url: &Url, budget: Duration) -> Result<Resolved, Refused> {
        if !self.shape(url) {
            return Err(Refused);
        }
        let Some(name) = url.host_str().filter(|host| literal(host).is_none()) else {
            return Ok(Resolved::Literal);
        };
        let port = url.port_or_known_default().ok_or(Refused)?;
        let found = match tokio::time::timeout(budget, self.lookup(name, port)).await {
            Ok(Ok(found)) if !found.is_empty() => found,
            _ => return Ok(Resolved::Nowhere),
        };
        if found.iter().any(|address| !self.allows(address.ip())) {
            return Err(Refused);
        }
        Ok(Resolved::Addresses(found))
    }

    /// 在本机解析一个主机名。测试打开了口子的，只照给的那张表，不问系统。
    async fn lookup(&self, name: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
        #[cfg(any(test, feature = "testkit"))]
        if let Some(testing) = &self.testing {
            return Ok(testing.lookup(name, port));
        }
        Ok(tokio::net::lookup_host((name, port)).await?.collect())
    }
}

/// 主机写的就是 IP 的，交回它：读网址时 IPv4 已经规整成点分的写法，IPv6 带着方括号。
fn literal(host: &str) -> Option<IpAddr> {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}

/// 是不是回环：`127.0.0.0/8`、`::1`，嵌着回环 IPv4 的 IPv6 也算。只给测试的口子用。
#[cfg(any(test, feature = "testkit"))]
fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => {
            ip.is_loopback() || ip.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// 是不是公网地址（第 3 层）：表照桥的 `guard.rs`，测性能的段 `198.18.0.0/15` 算公网（第 5 条）。
pub(crate) fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                // 0.0.0.0/8「这个网络」
                || a == 0
                // 100.64.0.0/10 运营商级 NAT
                || (a == 100 && (64..=127).contains(&b))
                // 192.0.0.0/24 IETF 协议用
                || (a == 192 && b == 0 && c == 0)
                // 240.0.0.0/4 保留
                || a >= 240)
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // 里面嵌着 IPv4 的，照那个 IPv4 判
            if let Some(v4) = ip.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                return is_public(IpAddr::V4(v4_of(s[6], s[7])));
            }
            if s[0] == 0x2002 {
                return is_public(IpAddr::V4(v4_of(s[1], s[2])));
            }
            !(
                // ::/96：未指定、回环、IPv4 兼容写法
                s[..6] == [0; 6]
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                // fec0::/10 站点本地（废了，有的系统还认）
                || (s[0] & 0xffc0) == 0xfec0
                // 2001:db8::/32、3fff::/20 文档
                || (s[0] == 0x2001 && s[1] == 0x0db8)
                || (s[0] == 0x3fff && s[1] < 0x1000)
                // 64:ff9b:1::/48 本地 NAT64
                || (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1)
                // 100::/64 丢弃
                || (s[0] == 0x100 && s[1..4] == [0; 3])
            )
        }
    }
}

/// 两段 16 位拼回一个 IPv4。
fn v4_of(high: u16, low: u16) -> Ipv4Addr {
    Ipv4Addr::from((u32::from(high) << 16) | u32::from(low))
}

#[cfg(test)]
mod tests;
