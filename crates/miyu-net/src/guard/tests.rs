use std::net::Ipv6Addr;

use super::*;
use crate::testkit::Testing;

/// 照桥的 `guard.rs` 测试搬过来的表；`198.18.0.0/15` 改成公网（`net.md`「怎么走」第 5 条）。
#[test]
fn only_public_addresses_pass() {
    let table = [
        // IPv4：公网
        ("8.8.8.8", true),
        ("1.1.1.1", true),
        ("93.184.216.34", true),
        ("100.63.255.255", true),
        ("100.128.0.1", true),
        ("172.15.255.255", true),
        ("172.32.0.1", true),
        ("192.0.1.1", true),
        ("192.169.0.1", true),
        ("198.17.255.255", true),
        ("198.20.0.1", true),
        ("223.255.255.255", true),
        // 测性能的段：Clash、mihomo 这类代理的假地址都在这里，当公网
        ("198.18.0.1", true),
        ("198.19.255.255", true),
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
        // 嵌着测性能的段的，照那个 IPv4 判：公网
        ("::ffff:198.18.0.1", true),
        ("64:ff9b::c612:1", true),
        ("2002:c612:1::1", true),
        // IPv6：不是公网
        ("::", false),
        ("::1", false),
        ("::ffff:127.0.0.1", false),
        ("::ffff:10.0.0.1", false),
        ("::ffff:192.168.0.1", false),
        ("::ffff:169.254.169.254", false),
        ("::ffff:100.64.0.1", false),
        ("::127.0.0.1", false),
        ("::8.8.8.8", false),
        ("64:ff9b::7f00:1", false),
        ("64:ff9b::a00:1", false),
        ("64:ff9b::c0a8:101", false),
        ("64:ff9b:1::1", false),
        ("2002:7f00:1::1", false),
        ("2002:a00:1::1", false),
        ("2002:c0a8:101::1", false),
        ("fe80::1", false),
        ("febf::1", false),
        ("fc00::1", false),
        ("fd12:3456::1", false),
        ("fec0::1", false),
        ("ff02::1", false),
        ("ff05::2", false),
        ("2001:db8::1", false),
        ("3fff::1", false),
        ("3fff:fff::1", false),
        ("100::1", false),
    ];
    for (text, public) in table {
        let ip: IpAddr = text.parse().expect(text);
        assert_eq!(is_public(ip), public, "{text}");
    }
    // 第 3 层的边上：3fff::/20 之外、100::/64 之外的是公网
    assert!(is_public(IpAddr::V6(
        "3fff:1000::1".parse::<Ipv6Addr>().unwrap()
    )));
    assert!(is_public(IpAddr::V6(
        "100:0:0:1::1".parse::<Ipv6Addr>().unwrap()
    )));
}

#[test]
fn url_shapes_that_never_leave() {
    let guard = Guard::new();
    for (text, safe) in [
        ("https://example.com/a?b=c", true),
        ("http://8.8.8.8/", true),
        ("http://[2606:4700:4700::1111]/", true),
        ("http://198.18.0.1/", true),
        ("http://localhost:8765/", false),
        ("http://LOCALHOST./", false),
        ("http://api.localhost/", false),
        ("http://printer.local/", false),
        ("http://printer.local./", false),
        ("http://user:pw@example.com/", false),
        ("http://user@example.com/", false),
        ("http://:pw@example.com/", false),
        ("ftp://example.com/", false),
        ("file:///etc/passwd", false),
        ("javascript:alert(1)", false),
        ("data:text/html,hi", false),
        ("http://127.1/", false),
        ("http://0x7f000001/", false),
        ("http://2130706433/", false),
        ("http://10.1.2.3:8080/", false),
        ("http://[::1]/", false),
        ("http://[::ffff:127.0.0.1]/", false),
        ("http://[::ffff:7f00:1]/", false),
        ("http://169.254.169.254/latest/meta-data/", false),
    ] {
        let url = Url::parse(text);
        assert_eq!(
            url.as_ref().is_ok_and(|url| guard.shape(url)),
            safe,
            "{text}"
        );
    }
}

#[tokio::test]
async fn literal_addresses_are_judged_without_dns() {
    // 一张空表：问到了 DNS 就是「解析不出来」，不会是 Literal
    let guard = Guard::testing(Testing::default());
    let budget = Duration::from_secs(60);
    for text in ["http://8.8.8.8/", "http://[2606:4700:4700::1111]:8443/x"] {
        let url = Url::parse(text).unwrap();
        assert_eq!(
            guard.pass(&url, budget).await,
            Ok(Resolved::Literal),
            "{text}"
        );
    }
    for text in [
        "http://127.0.0.1/",
        "http://[fd00::1]/",
        "http://localhost/",
        "gopher://example.com/",
    ] {
        let url = Url::parse(text).unwrap();
        assert_eq!(guard.pass(&url, budget).await, Err(Refused), "{text}");
    }
}

#[tokio::test]
async fn one_private_address_among_the_resolved_ones_refuses_the_whole_host() {
    let ip = |text: &str| text.parse::<IpAddr>().unwrap();
    let guard = Guard::testing(Testing {
        hosts: vec![
            ("public.test".to_string(), ip("8.8.8.8")),
            ("mixed.test".to_string(), ip("8.8.4.4")),
            ("mixed.test".to_string(), ip("10.0.0.1")),
            ("inner.test".to_string(), ip("192.168.1.1")),
            ("loop.test".to_string(), ip("127.0.0.1")),
            ("six.test".to_string(), ip("::ffff:10.0.0.1")),
            ("fake.test".to_string(), ip("198.18.0.7")),
        ],
        ..Testing::default()
    });
    let budget = Duration::from_secs(60);
    let pass = |text: &str| {
        let url = Url::parse(text).unwrap();
        let guard = guard.clone();
        async move { guard.pass(&url, budget).await }
    };
    assert_eq!(
        pass("https://public.test/").await,
        Ok(Resolved::Addresses(vec!["8.8.8.8:443".parse().unwrap()]))
    );
    assert_eq!(
        pass("http://fake.test:8080/").await,
        Ok(Resolved::Addresses(vec![
            "198.18.0.7:8080".parse().unwrap()
        ]))
    );
    for text in [
        "http://mixed.test/",
        "http://inner.test/",
        "http://loop.test/",
        "http://six.test/",
    ] {
        assert_eq!(pass(text).await, Err(Refused), "{text}");
    }
    // 表里没有：解析不出来，怎么办由走不走代理定
    assert_eq!(pass("http://nowhere.test/").await, Ok(Resolved::Nowhere));
}

#[tokio::test]
async fn the_loopback_switch_opens_only_loopback() {
    let ip = |text: &str| text.parse::<IpAddr>().unwrap();
    let guard = Guard::testing(Testing {
        loopback_public: true,
        hosts: vec![
            ("site.test".to_string(), ip("127.0.0.1")),
            ("inner.test".to_string(), ip("10.0.0.1")),
        ],
        ..Testing::default()
    });
    assert!(guard.allows(ip("127.0.0.1")));
    assert!(guard.allows(ip("::1")));
    assert!(guard.allows(ip("::ffff:127.0.0.1")));
    for text in [
        "10.0.0.1",
        "192.168.0.1",
        "169.254.169.254",
        "::",
        "fd00::1",
    ] {
        assert!(!guard.allows(ip(text)), "{text}");
    }
    let budget = Duration::from_secs(60);
    let url = Url::parse("http://inner.test/").unwrap();
    assert_eq!(guard.pass(&url, budget).await, Err(Refused));
    // 名字的那一层照旧：localhost 不去
    assert!(!guard.shape(&Url::parse("http://localhost/").unwrap()));
    // 平时的闸没有这个口子
    assert!(!Guard::new().allows(ip("127.0.0.1")));
}
