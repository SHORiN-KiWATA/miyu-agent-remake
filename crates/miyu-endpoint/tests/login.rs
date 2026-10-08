//! 身份（施工 W-8，`docs/blueprint/web-module.md`「怎么走」第一条）：一次性码只给出示本机令牌的连接、5 分钟、一次；用码连上的
//! 只能设用户名和密码，设好了换出登录令牌、落了盘才回、以前的登录全部作废；密码登录对的进、错的一样的话、同一个用户名
//! 60 秒错 5 次就先拒；登录令牌认得、过期、作废的不认；凭据只写一种；`account.logout` 作废一个、全部，断开用登录令牌、
//! 密码连着的连接；文件坏了的样子。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_session::testkit::Script;
use miyu_store::{accounts, logins};

use crate::support::login::*;
use crate::support::*;

/// 读 `system/accounts.json`。
fn accounts_of(home: &Home) -> accounts::Accounts {
    accounts::read(&home.root.system().join(accounts::FILE))
        .expect("读得了")
        .accounts
}

/// 读管理员的 `logins.json`。
fn logins_of(home: &Home) -> logins::Logins {
    logins::read(&home.root.account_dir(&alice()).join(logins::FILE))
        .expect("读得了")
        .logins
}

#[tokio::test]
async fn a_code_comes_only_from_the_local_token_and_only_sets_the_password() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let issued = setup_code(&core).await;
    let code = issued["code"].as_str().expect("有码");
    assert!(is_hex64(code), "{issued}");
    assert_eq!(issued["first"], true, "还没设过密码");
    assert!(issued["expires"].is_string(), "{issued}");
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"code": code})).await;
    assert_eq!(reply["result"]["account"], "alice", "{reply}");
    assert_eq!(reply["result"]["setup"], true);
    assert!(reply["result"].get("login").is_none());
    // 设好以前只能设密码；给人看的字照给。
    let refused = browser.call("l", "session.list", json!({})).await;
    assert_eq!(why(&refused), "setup_first", "{refused}");
    let human = browser.call("t", "human.get", json!({})).await;
    assert!(human.get("result").is_some(), "{human}");
    let refused = browser.call("c", "account.setup_code", json!({})).await;
    assert_eq!(why(&refused), "setup_first", "{refused}");
    // 写法不对的照旧是设密码的样子，可以再来。
    for (username, password) in [
        ("Shorin", "correct horse"),
        ("1st", "correct horse"),
        ("a-very-long-username-over-32-chars", "correct horse"),
        ("shorin", "short"),
        ("shorin", "        "),
    ] {
        let refused = browser
            .call(
                "s",
                "account.setup",
                json!({"username": username, "password": password}),
            )
            .await;
        assert_eq!(
            why(&refused),
            "bad_params",
            "{username} {password}: {refused}"
        );
    }
    let long = "x".repeat(1025);
    let refused = browser
        .call(
            "s",
            "account.setup",
            json!({"username": "shorin", "password": long}),
        )
        .await;
    assert_eq!(why(&refused), "bad_params", "{refused}");
    assert!(accounts_of(&home).accounts.is_empty(), "什么都没写");
    let reply = browser
        .call(
            "s",
            "account.setup",
            json!({"username": "shorin", "password": "correct horse"}),
        )
        .await;
    assert_eq!(reply["result"]["username"], "shorin", "{reply}");
    let token = reply["result"]["login"]["token"].as_str().expect("有令牌");
    assert!(is_hex64(token));
    assert!(reply["result"]["login"]["expires"].is_string());
    // 落了盘才回。
    let account = accounts_of(&home)
        .find("shorin")
        .cloned()
        .expect("写进去了");
    assert_eq!(account.id, "alice");
    assert!(accounts::verify(&account.password, "correct horse"));
    assert!(logins_of(&home).valid(
        &logins::digest(token),
        miyu_kernel::time::Timestamp::from_unix_millis(0).expect("合")
    ));
    // 设好了，这个连接是完整的管理员。
    let listed = browser.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "{listed}");
    let refused = browser
        .call(
            "s",
            "account.setup",
            json!({"username": "again", "password": "correct horse"}),
        )
        .await;
    assert_eq!(
        why(&refused),
        "bad_params",
        "不是用码连上的不能设：{refused}"
    );
    // 浏览器拿到登录令牌也换不出一次性码。
    let refused = browser.call("c", "account.setup_code", json!({})).await;
    assert_eq!(why(&refused), "local_only", "{refused}");
    // 系统日志记一条。
    let journal = std::fs::read_to_string(home.root.system().join(miyu_store::journal::FILE))
        .expect("有系统日志");
    let line = journal
        .lines()
        .find(|line| line.contains("\"account.password_set\""))
        .expect("记了一条");
    assert!(line.contains("\"username\":\"shorin\""), "{line}");
    assert!(
        !line.contains("correct horse") && !line.contains(token),
        "{line}"
    );
    // 用过的码不能再用。
    let mut again = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut again, json!({"code": code})).await;
    assert_eq!(why(&refused), "bad_code", "{refused}");
    assert!(is_closed(&mut again).await, "回完断开");
}

#[tokio::test]
async fn codes_expire_and_only_the_newest_sixteen_are_kept() {
    let home = Home::new();
    let core = home.core_code_ttl(Duration::from_millis(200));
    let stale = setup_code(&core).await["code"]
        .as_str()
        .expect("有码")
        .to_string();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut browser = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut browser, json!({"code": stale})).await;
    assert_eq!(why(&refused), "bad_code", "过期了：{refused}");
    let core = home.core(&Script::new([]));
    let mut client = local(&core).await;
    let mut codes = Vec::new();
    for n in 0..17 {
        let reply = client
            .call(&format!("c{n}"), "account.setup_code", json!({}))
            .await;
        codes.push(reply["result"]["code"].as_str().expect("有码").to_string());
    }
    let mut browser = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut browser, json!({"code": codes[0]})).await;
    assert_eq!(why(&refused), "bad_code", "多了丢最早的：{refused}");
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"code": codes[1]})).await;
    assert_eq!(reply["result"]["setup"], true, "{reply}");
}

#[tokio::test]
async fn passwords_log_in_and_too_many_failures_are_refused_without_checking() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    set_up(&core, "shorin", "correct horse").await;
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut browser,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    assert_eq!(reply["result"]["account"], "alice", "{reply}");
    let token = reply["result"]["login"]["token"].as_str().expect("有令牌");
    assert!(is_hex64(token));
    assert!(reply["result"].get("setup").is_none());
    let listed = browser.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "{listed}");
    // 用户名不对和密码不对一样说，回完断开。
    for (user, password) in [("shorin", "wrong horse"), ("nobody", "correct horse")] {
        let mut wrong = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut wrong, json!({"user": user, "password": password})).await;
        assert_eq!(why(&refused), "bad_password", "{refused}");
        assert!(is_closed(&mut wrong).await, "回完断开");
    }
    for _ in 0..4 {
        let mut wrong = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut wrong, json!({"user": "shorin", "password": "wrong"})).await;
        assert_eq!(why(&refused), "bad_password");
    }
    let mut right = Client::connect(Arc::clone(&core));
    let refused = hello_with(
        &mut right,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    assert_eq!(
        why(&refused),
        "login_throttled",
        "错了 5 次，对的也先拒：{refused}"
    );
    let wait = refused["error"]["data"]["retry_after_ms"]
        .as_u64()
        .expect("说要等多久");
    assert!(wait > 0 && wait <= 60_000, "{wait}");
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("miyu web --reset")
    );
}

#[tokio::test]
async fn login_tokens_are_known_until_they_expire_or_are_revoked() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let token = set_up(&core, "shorin", "correct horse").await;
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"login": token})).await;
    assert_eq!(reply["result"]["account"], "alice", "{reply}");
    assert!(reply["result"].get("login").is_none(), "用登录令牌的不另发");
    for bad in [&"0".repeat(64), "not hex"] {
        let mut stranger = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut stranger, json!({"login": bad})).await;
        assert_eq!(why(&refused), "bad_login", "{refused}");
        assert!(is_closed(&mut stranger).await);
    }
    // 过期了的不认：照文件写一个已经过期的。
    let path = home.root.account_dir(&alice()).join(logins::FILE);
    let read = logins::read(&path).expect("读得了");
    let mut written = read.logins.clone();
    let expired = "1".repeat(64);
    written.tokens.push(logins::Login {
        created: miyu_kernel::time::Timestamp::parse("2026-01-01T00:00:00.000Z").expect("合"),
        expires: miyu_kernel::time::Timestamp::parse("2026-01-31T00:00:00.000Z").expect("合"),
        hash: logins::digest(&expired),
    });
    logins::write(&path, &written, read.version.as_deref()).expect("写得进");
    let mut old = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut old, json!({"login": expired})).await;
    assert_eq!(why(&refused), "bad_login", "{refused}");
    // 重设密码：以前的登录、以前的密码都不认，新的认。
    let issued = setup_code(&core).await;
    assert_eq!(issued["first"], false, "设过了");
    let new_token = {
        let mut browser = Client::connect(Arc::clone(&core));
        hello_with(&mut browser, json!({"code": issued["code"]})).await;
        let reply = browser
            .call(
                "s",
                "account.setup",
                json!({"username": "admin", "password": "battery staple"}),
            )
            .await;
        reply["result"]["login"]["token"]
            .as_str()
            .expect("{reply}")
            .to_string()
    };
    let mut stale = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut stale, json!({"login": token})).await;
    assert_eq!(
        why(&refused),
        "bad_login",
        "重设以后以前的登录作废：{refused}"
    );
    assert!(is_closed(&mut browser).await, "以前登着的连接断开");
    let mut stale = Client::connect(Arc::clone(&core));
    let refused = hello_with(
        &mut stale,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    assert_eq!(why(&refused), "bad_password", "{refused}");
    let mut fresh = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut fresh, json!({"login": new_token})).await;
    assert_eq!(reply["result"]["account"], "alice", "{reply}");
    assert_eq!(
        accounts_of(&home).accounts.len(),
        1,
        "还是一个账号，用户名换了"
    );
    assert_eq!(accounts_of(&home).accounts[0].username, "admin");
}

#[tokio::test]
async fn logout_revokes_one_or_all_and_closes_their_connections() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let first = set_up(&core, "shorin", "correct horse").await;
    let mut a = Client::connect(Arc::clone(&core));
    hello_with(&mut a, json!({"login": first})).await;
    let mut b = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut b,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    let second = reply["result"]["login"]["token"]
        .as_str()
        .expect("有")
        .to_string();
    let mut c = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut c,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    assert!(reply.get("result").is_some());
    let mut terminal = local(&core).await;
    let reply = a.call("o", "account.logout", json!({})).await;
    assert_eq!(reply["result"]["revoked"], 1, "{reply}");
    assert!(is_closed(&mut a).await, "回完断开");
    let mut again = Client::connect(Arc::clone(&core));
    assert_eq!(
        why(&hello_with(&mut again, json!({"login": first})).await),
        "bad_login"
    );
    let listed = b.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "别的照旧：{listed}");
    // 本机的只能全部作废。
    let refused = terminal.call("o", "account.logout", json!({})).await;
    assert_eq!(why(&refused), "bad_params", "{refused}");
    let reply = b.call("o", "account.logout", json!({"all": true})).await;
    assert_eq!(reply["result"]["revoked"], 2, "{reply}");
    assert!(is_closed(&mut b).await);
    assert!(is_closed(&mut c).await, "用密码连着的也断开");
    let mut again = Client::connect(Arc::clone(&core));
    assert_eq!(
        why(&hello_with(&mut again, json!({"login": second})).await),
        "bad_login"
    );
    let listed = terminal.call("l", "session.list", json!({})).await;
    assert!(listed.get("result").is_some(), "本机的不受影响：{listed}");
    let reply = terminal
        .call("o", "account.logout", json!({"all": true}))
        .await;
    assert_eq!(reply["result"]["revoked"], 0, "{reply}");
    assert!(logins_of(&home).tokens.is_empty());
    // 密码不动。
    let mut d = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut d,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    assert!(reply.get("result").is_some(), "{reply}");
}

#[tokio::test]
async fn only_one_kind_of_credential_is_accepted() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let code = setup_code(&core).await["code"]
        .as_str()
        .expect("有")
        .to_string();
    for credentials in [
        json!({"token": TOKEN, "code": code}),
        json!({"user": "shorin"}),
        json!({"password": "correct horse"}),
        json!({"login": "a", "user": "b", "password": "c"}),
    ] {
        let mut client = Client::connect(Arc::clone(&core));
        let refused = hello_with(&mut client, credentials.clone()).await;
        assert_eq!(why(&refused), "bad_params", "{credentials}: {refused}");
        assert!(is_closed(&mut client).await, "回完断开");
    }
    let mut client = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut client, json!({})).await;
    assert_eq!(why(&refused), "bad_token", "{refused}");
    // 码没被上面那次一起写的握手用掉。
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(&mut browser, json!({"code": code})).await;
    assert_eq!(reply["result"]["setup"], true, "{reply}");
}

#[tokio::test]
async fn broken_files_read_as_empty() {
    let home = Home::new();
    home.write("system/accounts.json", "not json");
    let core = home.core(&Script::new([]));
    let issued = setup_code(&core).await;
    assert_eq!(issued["first"], true, "坏了的当没设过：{issued}");
    let mut browser = Client::connect(Arc::clone(&core));
    let refused = hello_with(
        &mut browser,
        json!({"user": "admin", "password": "whatever1"}),
    )
    .await;
    assert_eq!(why(&refused), "bad_password");
    let token = set_up(&core, "shorin", "correct horse").await;
    assert_eq!(accounts_of(&home).accounts.len(), 1, "坏的那份换成了好的");
    home.write("home/alice/logins.json", "{");
    let mut stale = Client::connect(Arc::clone(&core));
    let refused = hello_with(&mut stale, json!({"login": token})).await;
    assert_eq!(why(&refused), "bad_login", "{refused}");
    let mut browser = Client::connect(Arc::clone(&core));
    let reply = hello_with(
        &mut browser,
        json!({"user": "shorin", "password": "correct horse"}),
    )
    .await;
    assert!(
        reply["result"]["login"]["token"].is_string(),
        "坏的那份换成了好的：{reply}"
    );
    assert_eq!(logins_of(&home).tokens.len(), 1);
}
