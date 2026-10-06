//! 登录令牌的哈希（施工 W-8）：没有的是空的；加的时候删过期的、最多 64 行删最早造的；删一个、全删；认得没过期的、
//! 过期的不认；0600；坏了的说是坏了。

use std::fs;

use super::*;
use crate::test_support::Scratch;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("合写法")
}

fn login(token: &str, created: &str, expires: &str) -> Login {
    Login {
        created: at(created),
        expires: at(expires),
        hash: digest(token),
    }
}

#[test]
fn the_digest_is_sha256_hex() {
    // printf abc | sha256sum
    assert_eq!(
        digest("abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn adding_prunes_expired_and_keeps_at_most_64() {
    let now = at("2026-10-04T00:00:00.000Z");
    let mut logins = Logins::default();
    logins.add(
        login(
            "old",
            "2026-09-01T00:00:00.000Z",
            "2026-10-01T00:00:00.000Z",
        ),
        now,
    );
    logins.add(
        login("a", "2026-10-01T00:00:00.000Z", "2026-10-31T00:00:00.000Z"),
        now,
    );
    assert_eq!(logins.tokens.len(), 1, "过期的删了");
    assert!(logins.valid(&digest("a"), now));
    assert!(!logins.valid(&digest("old"), now));
    assert!(
        !logins.valid(&digest("a"), at("2026-10-31T00:00:00.000Z")),
        "到点就不认"
    );
    for n in 0..70 {
        let created = Timestamp::from_unix_millis(now.unix_millis() + n).expect("合");
        let expires = Timestamp::from_unix_millis(now.unix_millis() + 86_400_000).expect("合");
        logins.add(
            Login {
                created,
                expires,
                hash: digest(&format!("t{n}")),
            },
            now,
        );
    }
    assert_eq!(logins.tokens.len(), 64);
    assert!(!logins.valid(&digest("a"), now), "最早造的先删");
    assert!(!logins.valid(&digest("t5"), now));
    assert!(logins.valid(&digest("t6"), now));
    assert!(logins.valid(&digest("t69"), now));
    assert!(logins.remove(&digest("t69")));
    assert!(!logins.remove(&digest("t69")), "删过的没有了");
    assert_eq!(logins.clear(), 63);
    assert!(logins.tokens.is_empty());
}

#[test]
fn a_written_file_reads_back_private() {
    let temp = Scratch::new();
    let path = temp.path().join("home").join("admin").join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert!(empty.logins.tokens.is_empty());
    let mut logins = empty.logins;
    logins.add(
        login("a", "2026-10-04T00:00:00.000Z", "2026-11-03T00:00:00.000Z"),
        at("2026-10-04T00:00:00.000Z"),
    );
    write(&path, &logins, empty.version.as_deref()).expect("写得进");
    let again = read(&path).expect("读得回");
    assert_eq!(again.logins, logins);
    let text = fs::read_to_string(&path).expect("在");
    assert_eq!(
        text,
        format!(
            "{{\"version\":1,\"tokens\":[{{\"created\":\"2026-10-04T00:00:00.000Z\",\"expires\":\"2026-11-03T00:00:00.000Z\",\"hash\":\"{}\"}}]}}\n",
            digest("a")
        )
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).expect("在").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    fs::write(&path, "{\"version\":1,\"tokens\":[{\"hash\":1}]}").expect("写得进");
    assert!(read(&path).is_err());
}
