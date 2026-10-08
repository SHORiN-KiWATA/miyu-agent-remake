//! 系统账号（施工 O-4 下，`docs/construction/O-4-系统账号（下）.md`）：声明了系统账号的包，核心起来时建它的家目录和工作区；
//! 核心拉起的这个包的扩展以它的身份连进来：握手回应的 `account` 是它，群的场所会话归它（目录、工作目录、回应的 `account`），
//! 说完一轮用量记在它名下、记忆归管理员；主人的私聊照旧归主人，这个连接照样能对它说话；没声明系统账号的扩展照旧回
//! `no_system_account`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 包 `id` 的清单：程序 `program`、参数 `args`，`start`、`system_account` 照写。
fn install_serving(
    home: &Home,
    id: &str,
    program: &str,
    args: &[String],
    start: &str,
    system_account: bool,
) {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        &format!("home/alice/packages/{id}.toml"),
        &format!(
            "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"B\" }}\n\n[process]\nargs = [{}]\nstart = \"{start}\"\nsystem_account = {system_account}\n",
            args.join(", ")
        ),
    );
}

/// 照对应表、读好的清单起来的核心，拉起开着的扩展。
fn served_core(home: &Home, script: &Script) -> Arc<Core> {
    home.write("system/config.toml", BINDINGS);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 等到记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
async fn replies(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn bot() -> AccountId {
    AccountId::parse("bot").expect("合写法")
}

/// 会话 `session` 在账号 `account` 名下的目录。
fn dir(home: &Home, account: &AccountId, session: &str) -> std::path::PathBuf {
    home.root
        .session_dir(account, &SessionId::parse(session).expect("会话编号合写法"))
}

/// 等到 `dir` 里的日志结束了 `n` 轮。
async fn until_turns_in(dir: &Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let ended = read_events(dir)
            .unwrap_or_default()
            .iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count();
        if ended >= n {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到第 {n} 轮");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 只读打开 SQLite 库 `db`，照 `sql`（带一个参数 `value`）数到大于 0 为止，最多 60 秒。库还没建、表还没有的当 0。
async fn until_counted(db: &Path, sql: &str, value: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let counted =
            rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .and_then(|db| db.query_row(sql, [value], |row| row.get::<_, i64>(0)))
                .unwrap_or(0);
        if counted > 0 {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} 里数不到：{sql}",
            db.display()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn the_bridge_runs_as_its_system_account_and_owns_the_group() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let group = r#"{"venue":"qq:group:1","kind":"group","persona":"engineer"}"#;
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:venue.session:{group}"),
            &format!("ask:venue.session:{group}"),
            r#"ask:session.send:{"session":"{session}","text":"我的显卡是 N 卡","as":{"external":"qq:20001"}}"#,
            r#"ask:session.send:{"session":"{session}","text":"我也在","as":{"external":"qq:10001"}}"#,
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001"}"#,
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:10001"}}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let script = Script::new([
        Play::Says("好，记住你用 N 卡。"),
        Play::Says("嗯。"),
        Play::Says("在。"),
        Play::Says("在。"),
        Play::Says("嗯。"),
    ]);
    let core = served_core(&home, &script);
    let got = replies(&path, 7).await;

    assert_eq!(
        got[0]["result"]["account"], "bot",
        "握手：这个连接是系统账号"
    );
    let made = &got[1]["result"];
    assert_eq!(made["created"], true, "{got:?}");
    assert_eq!(made["account"], "bot", "群归系统账号");
    let session = made["session"].as_str().expect("有编号").to_string();
    assert_eq!(
        got[2]["result"],
        json!({"session": session, "created": false, "account": "bot"}),
        "照「场所加属主」找回同一个"
    );
    assert!(got[3].get("error").is_none(), "{got:?}");
    assert!(got[4].get("error").is_none(), "{got:?}");
    let groups = dir(&home, &bot(), &session);
    let created = read_events(&groups).expect("目录在 bot 的家目录下");
    let Body::SessionCreated(created) = &created[0].body else {
        panic!("第一条是 session.created");
    };
    assert_eq!(created.owner, bot());
    let workspace = home.root.workspace(&bot());
    assert_eq!(
        created.cwd.as_deref().map(Path::new),
        Some(workspace.as_path()),
        "没写 cwd 的是它自己的工作区"
    );
    assert!(workspace.is_dir(), "起来时建了它的工作区");
    // 群归系统账号以后，对应表认出的主人在群里说的是外部身份带账号，不是本人（`as` 照会话的属主比）。
    let owner_said = read_events(&groups)
        .expect("读得了")
        .into_iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by)
        .find(|by| matches!(by, By::External(external) if external.id.as_str() == "qq:10001"));
    assert!(
        matches!(&owner_said, Some(By::External(external)) if external.account == Some(alice())),
        "{owner_said:?}"
    );

    let private = &got[5]["result"];
    assert_eq!(
        private["account"], "alice",
        "对应表里的主人的私聊照旧归主人：{got:?}"
    );
    let mine = private["session"].as_str().expect("有编号").to_string();
    assert!(dir(&home, &alice(), &mine).is_dir());
    assert!(
        got[6].get("error").is_none(),
        "系统账号的连接照样能对主人的会话说话：{got:?}"
    );

    until_turns_in(&groups, 1).await;
    // 会话落盘时就写：索引在它自己的家目录下、行里属主是它；用量记在它名下（查之前，不靠 `usage.query` 补）。
    let index = home.root.index(&bot()).join("sessions.db");
    until_counted(
        &index,
        "SELECT count(*) FROM sessions WHERE id = ?1 AND owner = 'bot'",
        &session,
    )
    .await;
    let ledger = home.root.state().join("usage.db");
    until_counted(
        &ledger,
        "SELECT count(*) FROM spent WHERE session = ?1 AND owner = 'bot'",
        &session,
    )
    .await;
    let mut client = Client::connect(core.clone());
    client.hello().await;
    // 附件存在连接的账号名下，她照会话的属主读：拷一份进系统账号的 blob。
    let put = client
        .call(
            "b1",
            "blob.put",
            json!({"data": "aGVsbG8=", "name": "a.txt"}),
        )
        .await;
    let blob = put["result"]["blob"]
        .as_str()
        .unwrap_or_else(|| panic!("{put}"))
        .to_string();
    let attached = client
        .call(
            "b2",
            "session.send",
            json!({"session": session, "text": "看这个", "as": {"external": "qq:20002"},
                   "attachments": [{"blob": blob, "name": "a.txt", "media_type": put["result"]["media_type"]}]}),
        )
        .await;
    assert!(attached.get("error").is_none(), "{attached}");
    let hash = miyu_kernel::id::ContentHash::parse(&blob).expect("合写法");
    assert!(
        Blobs::new(home.root.blobs(&bot())).path(&hash).is_file(),
        "附件拷进了会话的属主名下"
    );
    let usage = client
        .call("u1", "usage.query", json!({"group": ["account"]}))
        .await;
    let accounts: Vec<&str> = usage["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{usage}"))
        .iter()
        .filter_map(|row| row["account"].as_str())
        .collect();
    assert!(
        accounts.contains(&"bot"),
        "用量记在系统账号名下、管理员看得到：{usage}"
    );

    // 记忆归管理员，系统账号没有自己的记忆。群里的回合先不进回合库（施工 R-2 再补，`memory.md` 第一条第 1 款：回合库的
    // 条目还没有听众），哪边都不为它建库。管理员那一间的回合库平常是本机的会话建的：这里直接建一份，看删群会话时是不是
    // 照记忆归谁去埋墓碑。
    let turns = |account: &AccountId| {
        home.root
            .index(account)
            .join("recall")
            .join("turns-engineer.db")
    };
    assert!(!turns(&bot()).exists(), "系统账号没有自己的回合库");
    assert!(!turns(&alice()).exists(), "群里的回合不进管理员的回合库");
    std::fs::create_dir_all(turns(&alice()).parent().expect("有上一级")).expect("建得了");
    drop(miyu_store::recall::RecallIndex::open(&turns(&alice())));
    // 删掉群会话：管理员的回合库里给它埋墓碑，记忆的出处在它里面的都算死了。
    // 带附件的那一句排出的那一轮可能还在跑：跑着的删不掉（`turn_running`），等它说完再删。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut n = 0;
    let deleted = loop {
        n += 1;
        let deleted = client
            .call(
                &format!("d{n}"),
                "session.delete",
                json!({"session": session}),
            )
            .await;
        if reason(&deleted) != Some("turn_running") {
            break deleted;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "一直在跑：{deleted}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert_eq!(deleted["result"], json!({}), "{deleted}");
    let db = rusqlite::Connection::open_with_flags(
        turns(&alice()),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("回合库在");
    let buried: i64 = db
        .query_row(
            "SELECT count(*) FROM buried WHERE key = ?1",
            [format!("{session}/")],
            |row| row.get(0),
        )
        .expect("查得了");
    assert_eq!(buried, 1, "照记忆归谁去拿掉");
    let left: i64 =
        rusqlite::Connection::open_with_flags(&index, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .and_then(|db| {
                db.query_row(
                    "SELECT count(*) FROM sessions WHERE id = ?1",
                    [&session],
                    |row| row.get(0),
                )
            })
            .expect("索引在");
    assert_eq!(left, 0, "它自己的索引里那一行拿掉了");
    core.stop_extensions().await;
}

#[tokio::test]
async fn an_extension_without_a_system_account_still_gets_no_group() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "plain");
    install_serving(
        &home,
        "plain",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
            "wait",
        ]),
        "always",
        false,
    );
    // 声明了系统账号、关着没拉起的：起来时照样建它的家目录和工作区。
    install_serving(&home, "quiet", &program.name(), &[], "manual", true);
    let core = served_core(&home, &Script::new([]));
    let quiet = AccountId::parse("quiet").expect("合写法");
    assert!(home.root.workspace(&quiet).is_dir(), "起来时建了它的工作区");
    let got = replies(&path, 2).await;
    assert_eq!(got[0]["result"]["account"], "alice", "没声明的照旧是管理员");
    assert_eq!(reason(&got[1]), Some("no_system_account"), "{got:?}");
    assert!(
        !home
            .root
            .account_dir(&AccountId::parse("plain").expect("合写法"))
            .exists(),
        "没声明的不建账号"
    );
    core.stop_extensions().await;
}

/// 用量汇总丢了（派生的，删掉重建）：`usage.query` 先补，系统账号名下的会话也照日志补回来。
#[tokio::test]
async fn usage_of_a_system_account_is_caught_up_after_the_ledger_is_lost() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let script = Script::new([Play::Says("好。")]);
    let bridge = |start: &str| {
        install_serving(
            &home,
            "bot",
            &program.name(),
            &steps(&[
                &step,
                "hello",
                r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
                r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:20001"}}"#,
                "wait",
            ]),
            start,
            true,
        );
    };
    bridge("always");
    let first = served_core(&home, &script);
    let got = replies(&path, 3).await;
    let session = got[1]["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{got:?}"))
        .to_string();
    until_turns_in(&dir(&home, &bot(), &session), 1).await;
    first.stop_extensions().await;
    first.stop_sessions().await;
    drop(first);
    // 删掉汇总再起来：关着不拉起，只剩日志。Windows 上开着的删不掉，照旧的读也一样过。
    for name in ["usage.db", "usage.db-wal", "usage.db-shm"] {
        drop(std::fs::remove_file(home.root.state().join(name)));
    }
    bridge("manual");
    let second = served_core(&home, &Script::new([]));
    let mut client = Client::connect(second.clone());
    client.hello().await;
    let usage = client
        .call("u1", "usage.query", json!({"group": ["account"]}))
        .await;
    let accounts: Vec<&str> = usage["result"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{usage}"))
        .iter()
        .filter_map(|row| row["account"].as_str())
        .collect();
    assert!(accounts.contains(&"bot"), "照它的日志补回来：{usage}");
}

/// `events.append`（施工 O-13 上）：核心拉起的扩展只能写自己的包那一段的 `ext.*`。
#[tokio::test]
async fn an_extension_writes_only_its_own_ext_events() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
            r#"ask:events.append:{"session":"{session}","kind":"ext.bot.chat.decided","body":{"to":[1]}}"#,
            r#"ask:events.append:{"session":"{session}","kind":"ext.other.chat.decided","body":{}}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let core = served_core(&home, &Script::new([]));
    let got = replies(&path, 4).await;
    assert!(got[2]["result"]["seq"].is_u64(), "自己的包：{got:?}");
    assert_eq!(reason(&got[3]), Some("bad_params"), "别的包的：{got:?}");
    let session = got[1]["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let decided = read_events(&dir(&home, &bot(), &session))
        .expect("读得了")
        .into_iter()
        .find(|event| event.body.kind() == "ext.bot.chat.decided")
        .expect("记下了");
    assert_eq!(
        serde_json::to_value(&decided.by).expect("写得出"),
        json!({"kind": "module", "id": "bot"}),
        "记成这个包写的"
    );
    core.stop_extensions().await;
}
