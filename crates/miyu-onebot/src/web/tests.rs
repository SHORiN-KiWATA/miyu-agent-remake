//! 验过的登录令牌（施工 O-16，`onebot.md` 第二条「施工时定的」第 3 条）：记一阵、过了就忘；只记哈希，不留原文。

use std::time::{Duration, Instant};

use super::checked::{Checked, hash};

const TOKEN: &str = "5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e";

#[test]
fn a_checked_token_is_kept_for_a_while_and_then_forgotten() {
    let checked = Checked::new(Duration::from_secs(60));
    let at = Instant::now();
    assert!(!checked.fresh(TOKEN, at), "还没验过");
    checked.remember(TOKEN, at);
    assert!(checked.fresh(TOKEN, at));
    assert!(checked.fresh(TOKEN, at + Duration::from_secs(59)));
    assert!(
        !checked.fresh("another", at + Duration::from_secs(1)),
        "别的令牌不算"
    );
    assert!(
        !checked.fresh(TOKEN, at + Duration::from_secs(60)),
        "到点就忘"
    );
    assert!(checked.keys().is_empty(), "过了时候的扔掉了");
}

#[test]
fn only_the_hash_is_kept() {
    let checked = Checked::new(Duration::from_secs(60));
    checked.remember(TOKEN, Instant::now());
    let keys = checked.keys();
    assert_eq!(keys, [hash(TOKEN)]);
    assert!(keys.iter().all(|key| !key.contains(TOKEN) && key != TOKEN));
    assert_eq!(
        hash("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "SHA-256 的十六进制"
    );
}
