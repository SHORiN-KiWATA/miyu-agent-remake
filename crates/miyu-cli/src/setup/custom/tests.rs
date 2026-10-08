//! 自定义的一家的编号照主机名起（施工 8-11 再补）。

use super::*;

#[test]
fn the_id_comes_from_the_host() {
    for (url, id) in [
        ("https://api.example.com/v1", "example"),
        ("https://open.bigmodel.cn/api/paas/v4", "bigmodel"),
        ("https://api.example.co.uk/v1", "example"),
        ("http://127.0.0.1:8000/v1", "local"),
        ("http://localhost:11434/v1", "local"),
        ("http://[::1]:8080/v1", "local"),
        ("https://user@gateway.example.org:8443/v1", "example"),
        ("https://relay/v1", "relay"),
        ("https://302.ai/v1", "p-302"),
    ] {
        assert_eq!(host_id(url), id, "{url}");
    }
    assert_eq!(
        host("https://user@gateway.example.org:8443/v1"),
        "gateway.example.org"
    );
}
