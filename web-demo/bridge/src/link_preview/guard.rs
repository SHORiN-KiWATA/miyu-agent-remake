//! 出站抓取的地址闸（照旧版 `miyu-engine` 的 `tools/net_guard.rs`）：链接卡片的地址是模型写的，这里等于把 SSRF 的靶子
//! 摆在桥上。
//!
//! 三层依次过：
//!
//! 1. 地址的样子：只认 http、https；不带用户名密码；`localhost`、`*.localhost`、`*.local` 不去；主机写的就是 IP 的，
//!    照第三层判（`127.1`、`0x7f000001`、`2130706433` 这些写法 URL 解析时已经规整成 `127.0.0.1`）。
//! 2. DNS：解析出来的每一个地址都得是公网的，有一个不是就整个不去。
//! 3. IP 段：回环、私网、链路本地、CGNAT、唯一本地、组播、未指定、广播、保留、文档段、测性能段不算公网；
//!    里面嵌着 IPv4 的 IPv6 写法（映射 `::ffff:a.b.c.d`、NAT64 `64:ff9b::/96`、6to4 `2002::/16`）照那个 IPv4 判，
//!    IPv4 兼容写法（`::a.b.c.d`，早废了）不去。
//!
//! 解析好的地址交回去，由 `fetch.rs` 钉进这一跳的客户端（`ClientBuilder::resolve_to_addrs`）：查过的就是连上的，
//! 中间没有第二次 DNS，对面没法在「查」和「连」之间把名字换成内网地址（DNS rebinding）。

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use reqwest::Url;

use super::Miss;

/// 这一跳要钉住的主机名和它解析出来的地址。
pub(super) type Pinned = (String, Vec<SocketAddr>);

/// 地址的样子过不过得了闸（不查 DNS）：第一层，主机写的是 IP 的连第三层一起判。
pub(super) fn is_safe_url(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']').trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return false;
    }
    match host.parse::<IpAddr>() {
        Ok(ip) => is_public(ip),
        Err(_) => true,
    }
}

/// 过闸，交回这一跳要钉住的地址；主机写的就是 IP 的交回 `None`（照它连，上面已经判过）。
///
/// # Errors
///
/// 样子不合规、没有主机或端口、解析出不是公网的地址：[`Miss::NoPreview`]（不会自己变好）；
/// 解析超时、解析不了：[`Miss::Transient`]（下次可能就好了）。
pub(super) async fn resolve(url: &Url, budget: Duration) -> Result<Option<Pinned>, Miss> {
    if !is_safe_url(url) {
        return Err(Miss::NoPreview);
    }
    let host = url.host_str().ok_or(Miss::NoPreview)?;
    if host.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>().is_ok() {
        return Ok(None);
    }
    let port = url.port_or_known_default().ok_or(Miss::NoPreview)?;
    let found = tokio::time::timeout(budget, tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| Miss::Transient)?
        .map_err(|_| Miss::Transient)?;
    let addresses: Vec<SocketAddr> = found.collect();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public(address.ip())) {
        return Err(Miss::NoPreview);
    }
    Ok(Some((host.to_string(), addresses)))
}

/// 是不是公网地址（第三层）。
pub(super) fn is_public(ip: IpAddr) -> bool {
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
                // 198.18.0.0/15 测性能用
                || (a == 198 && matches!(b, 18 | 19))
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
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_pass() {
        let table = [
            // IPv4：公网
            ("8.8.8.8", true),
            ("1.1.1.1", true),
            ("93.184.216.34", true),
            ("100.63.255.255", true),
            ("100.128.0.1", true),
            ("172.32.0.1", true),
            ("192.0.1.1", true),
            ("198.20.0.1", true),
            ("223.255.255.255", true),
            // IPv4：不是公网
            ("0.0.0.0", false),
            ("0.1.2.3", false),
            ("10.0.0.1", false),
            ("127.0.0.1", false),
            ("127.255.255.254", false),
            ("169.254.169.254", false),
            ("172.16.0.1", false),
            ("172.31.255.255", false),
            ("192.168.1.1", false),
            ("100.64.0.1", false),
            ("100.127.255.255", false),
            ("192.0.0.8", false),
            ("192.0.2.1", false),
            ("198.51.100.1", false),
            ("203.0.113.1", false),
            ("198.18.0.1", false),
            ("198.19.255.255", false),
            ("224.0.0.1", false),
            ("239.255.255.250", false),
            ("240.0.0.1", false),
            ("255.255.255.255", false),
            // IPv6：公网
            ("2606:4700:4700::1111", true),
            ("2001:4860:4860::8888", true),
            ("2400:cb00::1", true),
            ("::ffff:8.8.8.8", true),
            ("64:ff9b::808:808", true),
            ("2002:808:808::1", true),
            // IPv6：不是公网
            ("::", false),
            ("::1", false),
            ("::ffff:127.0.0.1", false),
            ("::ffff:10.0.0.1", false),
            ("::ffff:192.168.0.1", false),
            ("::ffff:169.254.169.254", false),
            ("::127.0.0.1", false),
            ("::8.8.8.8", false),
            ("64:ff9b::7f00:1", false),
            ("64:ff9b::a00:1", false),
            ("64:ff9b:1::1", false),
            ("2002:7f00:1::1", false),
            ("2002:c0a8:101::1", false),
            ("fe80::1", false),
            ("fc00::1", false),
            ("fd12:3456::1", false),
            ("fec0::1", false),
            ("ff02::1", false),
            ("ff05::2", false),
            ("2001:db8::1", false),
            ("3fff::1", false),
            ("100::1", false),
        ];
        for (text, public) in table {
            let ip: IpAddr = text.parse().expect(text);
            assert_eq!(is_public(ip), public, "{text}");
        }
    }

    #[test]
    fn url_shapes_that_never_leave() {
        for (text, safe) in [
            ("https://example.com/a?b=c", true),
            ("http://8.8.8.8/", true),
            ("http://[2606:4700:4700::1111]/", true),
            ("http://localhost:8765/", false),
            ("http://LOCALHOST./", false),
            ("http://api.localhost/", false),
            ("http://printer.local/", false),
            ("http://user:pw@example.com/", false),
            ("http://user@example.com/", false),
            ("ftp://example.com/", false),
            ("file:///etc/passwd", false),
            ("javascript:alert(1)", false),
            ("http://127.1/", false),
            ("http://0x7f000001/", false),
            ("http://2130706433/", false),
            ("http://10.1.2.3:8080/", false),
            ("http://[::1]/", false),
            ("http://[::ffff:127.0.0.1]/", false),
            ("http://169.254.169.254/latest/meta-data/", false),
        ] {
            let url = Url::parse(text);
            assert_eq!(url.as_ref().is_ok_and(is_safe_url), safe, "{text}");
        }
    }

    #[tokio::test]
    async fn literal_addresses_are_judged_without_dns() {
        let budget = Duration::from_millis(1);
        let public = Url::parse("http://8.8.8.8/").expect("写死的地址");
        assert_eq!(resolve(&public, budget).await, Ok(None));
        for text in ["http://127.0.0.1/", "http://[fd00::1]/", "http://localhost/", "gopher://example.com/"] {
            let url = Url::parse(text).expect(text);
            assert_eq!(resolve(&url, budget).await, Err(Miss::NoPreview), "{text}");
        }
    }
}
