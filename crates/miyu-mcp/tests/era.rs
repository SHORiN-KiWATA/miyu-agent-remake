//! 认时代（`docs/blueprint/mcp.md`「怎么走」第一条）：新时代答 `server/discover`，之后每个请求带 `_meta`；旧时代回别的错、
//! 不回的退回 `initialize`，认得的旧版本照它说；规范自己的错照报、不退回；缓存说是旧时代的直接握手，握不成再探。

use serde_json::Value;

use miyu_mcp::testkit::{Kind, Script};
use miyu_mcp::{Era, Failed};

use crate::support::{connect, connected, methods};

/// 这一条的 `_meta` 里带的版本。
fn version(message: &Value) -> Option<&str> {
    message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str()
}

#[tokio::test]
async fn a_modern_server_answers_the_probe_and_every_request_carries_its_version() {
    let (client, fake) = connected(Script::modern()).await;
    assert_eq!(client.era(), &Era::Modern("2026-07-28".to_string()));
    client.tools().await.expect("列得出");
    assert_eq!(fake.methods(), ["server/discover", "tools/list"]);
    let received = fake.received();
    assert_eq!(version(&received[1]), Some("2026-07-28"));
    assert_eq!(
        received[1]["params"]["_meta"]["io.modelcontextprotocol/clientInfo"]["name"],
        "miyu"
    );
}

#[tokio::test]
async fn a_modern_server_without_our_version_is_refused_with_its_list() {
    let script = Script {
        versions: vec!["2099-01-01".to_string()],
        ..Script::modern()
    };
    let (client, fake) = connect(script, None).await;
    assert_eq!(
        client.err(),
        Some(Failed::NoCommonVersion(vec!["2099-01-01".to_string()]))
    );
    assert_eq!(fake.methods(), ["server/discover"], "规范的错不退回握手");
}

#[tokio::test]
async fn a_modern_error_is_reported_not_taken_as_legacy() {
    let script = Script {
        kind: Kind::ModernDemands,
        ..Script::modern()
    };
    let (client, fake) = connect(script, None).await;
    assert!(matches!(
        client.err(),
        Some(Failed::Rpc { code: -32021, .. })
    ));
    assert_eq!(fake.methods(), ["server/discover"]);
}

#[tokio::test]
async fn a_legacy_server_refusing_the_probe_is_shaken_hands_with() {
    let (client, fake) = connected(Script::legacy(Kind::LegacyRefuses)).await;
    assert_eq!(client.era(), &Era::Legacy("2025-11-25".to_string()));
    client.tools().await.expect("列得出");
    assert_eq!(
        fake.methods(),
        [
            "server/discover",
            "initialize",
            "notifications/initialized",
            "tools/list"
        ]
    );
    let received = fake.received();
    assert_eq!(received[1]["params"]["protocolVersion"], "2025-11-25");
    assert_eq!(received[3]["params"].get("_meta"), None, "旧时代不带 _meta");
}

#[tokio::test]
async fn a_legacy_server_ignoring_the_probe_is_shaken_hands_with_after_the_wait() {
    let (client, fake) = connected(Script::legacy(Kind::LegacySilent)).await;
    assert_eq!(client.era(), &Era::Legacy("2025-11-25".to_string()));
    assert_eq!(
        methods(&fake, 3).await,
        ["server/discover", "initialize", "notifications/initialized"]
    );
}

#[tokio::test]
async fn an_older_legacy_version_we_know_is_spoken() {
    let script = Script {
        versions: vec!["2025-06-18".to_string()],
        ..Script::legacy(Kind::LegacyRefuses)
    };
    let (client, _) = connected(script).await;
    assert_eq!(client.era(), &Era::Legacy("2025-06-18".to_string()));
}

#[tokio::test]
async fn a_legacy_version_we_do_not_know_is_refused() {
    let script = Script {
        versions: vec!["1999-01-01".to_string()],
        ..Script::legacy(Kind::LegacyRefuses)
    };
    let (client, _) = connect(script, None).await;
    assert_eq!(
        client.err(),
        Some(Failed::NoCommonVersion(vec!["1999-01-01".to_string()]))
    );
}

#[tokio::test]
async fn a_legacy_server_dying_on_the_probe_is_a_closed_connection() {
    let (client, _) = connect(Script::legacy(Kind::LegacyDies), None).await;
    assert_eq!(client.err(), Some(Failed::Closed));
}

#[tokio::test]
async fn a_known_legacy_server_is_not_probed() {
    let known = Some(Era::Legacy("2025-11-25".to_string()));
    let (client, fake) = connect(Script::legacy(Kind::LegacyDies), known).await;
    let client = client.expect("不探就不会退出");
    assert_eq!(client.era(), &Era::Legacy("2025-11-25".to_string()));
    assert_eq!(
        methods(&fake, 2).await,
        ["initialize", "notifications/initialized"]
    );
}

#[tokio::test]
async fn a_server_no_longer_legacy_is_probed_again() {
    let known = Some(Era::Legacy("2025-11-25".to_string()));
    let (client, fake) = connect(Script::modern(), known).await;
    assert_eq!(
        client.expect("认得出").era(),
        &Era::Modern("2026-07-28".to_string())
    );
    assert_eq!(fake.methods(), ["initialize", "server/discover"]);
}

#[tokio::test]
async fn instructions_are_kept_from_either_era() {
    for script in [Script::modern(), Script::legacy(Kind::LegacyRefuses)] {
        let script = Script {
            instructions: Some("Use it well.".to_string()),
            ..script
        };
        let (client, _) = connected(script).await;
        assert_eq!(client.instructions(), Some("Use it well."));
    }
}
