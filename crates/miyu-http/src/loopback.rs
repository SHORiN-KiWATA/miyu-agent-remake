//! 回环地址（`docs/blueprint/http.md`「客户端」第 5 条，施工 8-11 补）：环境变量里的代理不会自动放行回环地址，供应商的
//! 地址落在本机时该直连——不然 `NO_PROXY` 没写回环地址的机器，探不到本机的服务，配好了也连不上。

use std::net::IpAddr;

/// 主机名是不是回环：`localhost`（大小写不论）、`*.localhost`，或者是回环的 IP（`127.0.0.0/8`、`::1`，IPv4 映射的
/// 也算，例如 `::ffff:127.0.0.1`）。`host` 带不带方括号（IPv6 字面量常见的写法）都认。
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim();
    let bare = host
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => v4.is_loopback(),
            IpAddr::V6(v6) => {
                v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
            }
        };
    }
    let lower = host.to_ascii_lowercase();
    lower == "localhost" || lower.ends_with(".localhost")
}

/// 地址（例如 `http://127.0.0.1:1234/v1`）的主机是不是回环；读不出主机名的（写法不对）当不是：照旧走代理，和以前
/// 读不出地址时的样子一样。
pub fn is_loopback_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(is_loopback_host))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_are_recognized() {
        for host in [
            "127.0.0.1",
            "127.1.2.3",
            "127.255.255.255",
            "localhost",
            "LOCALHOST",
            "LoCaLhOsT",
            "foo.localhost",
            "[::1]",
            "::1",
            "[::ffff:127.0.0.1]",
            "::ffff:127.0.0.1",
        ] {
            assert!(is_loopback_host(host), "{host} 该是回环");
        }
    }

    #[test]
    fn non_loopback_hosts_are_not() {
        for host in [
            "10.0.0.1",
            "192.168.1.1",
            "example.com",
            "localhost.example.com",
            "localhosts",
            "[::2]",
            "",
        ] {
            assert!(!is_loopback_host(host), "{host} 不该是回环");
        }
    }

    #[test]
    fn urls_are_read_by_their_host() {
        assert!(is_loopback_url("http://127.0.0.1:1234/v1"));
        assert!(is_loopback_url("https://localhost:8080/chat/completions"));
        assert!(is_loopback_url("http://[::1]:11434/v1"));
        assert!(!is_loopback_url("https://api.deepseek.com/v1"));
        assert!(!is_loopback_url("not a url"));
        assert!(!is_loopback_url(""));
    }
}
