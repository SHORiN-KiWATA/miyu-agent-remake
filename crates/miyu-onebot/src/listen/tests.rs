//! 升级的回应（`onebot.md` 第一条「怎么走」第 2 条）：算出来的 `Sec-WebSocket-Accept` 放不进头的，回 400、不升级。

use hyper::StatusCode;
use hyper::header;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;

use super::switching;

#[test]
fn a_good_accept_key_switches_protocols() {
    // RFC 6455 第 1.3 节的例子。
    let accept = derive_accept_key(b"dGhlIHNhbXBsZSBub25jZQ==");
    let response = switching(&accept);
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    assert_eq!(
        response.headers()[header::SEC_WEBSOCKET_ACCEPT],
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn an_accept_key_that_is_not_a_header_value_is_refused() {
    for accept in ["bad\nvalue", "bad\rvalue", "\u{0}"] {
        let response = switching(accept);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{accept:?}");
        assert!(
            response.headers().get(header::UPGRADE).is_none(),
            "{accept:?}：不说升级"
        );
    }
}
