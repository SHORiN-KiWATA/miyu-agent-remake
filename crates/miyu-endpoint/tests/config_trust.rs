//! 项目配置的信任（`docs/blueprint/config.md`「怎么走」第三条）：没有记录的、内容变了的、仓库挪了的不算，报
//! `untrusted_project`，造会话、说话的回应带它；信任着、版本一样的算，只认收紧的；选了不信任的不算、不再提醒（施工 8-2，
//! 记录由人手写）。`config.trust`（施工 8-3）：版本对不上、没有项目配置、参数不对的拒绝；一个仓库一条，新的盖掉旧的；
//! 记账号日志 `trust.changed`；手改过的 `trust.toml` 在新的字上记。

mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_store::config_file::version;

use support::*;

/// 项目配置开局只读的一份。
const STRICT: &str = "# 这个仓库里开的新会话一开始只读\n[permission]\nstart_read_only = true\n";

/// 在工作目录下建一个仓库 `name`（有 `.git`），放一份项目配置 `text`，交回仓库在哪。
fn repo(home: &Home, name: &str, text: &str) -> PathBuf {
    let repo = home.work.join(name);
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::create_dir_all(repo.join(".miyu")).expect("建得了");
    std::fs::write(repo.join(".miyu").join("config.toml"), text).expect("写得进");
    repo
}

/// 记一条信任的记录：仓库 `repo`，版本照 `text` 算。
fn record(home: &Home, repo: &Path, text: &str, trusted: bool) {
    let path = repo.to_string_lossy().replace('\\', "\\\\");
    home.write(
        "home/alice/trust.toml",
        &format!(
            "[[project]]\npath = \"{path}\"\nversion = \"{}\"\ntrusted = {trusted}\n",
            version(text.as_bytes())
        ),
    );
}

/// 造一个会话、说一句，交回造会话的回应、说话的回应，和它开局是不是只读。
async fn open(home: &Home, cwd: &Path) -> (Value, Value, bool) {
    // 每次一个新的命令编号：同一个数据根上重起的核心认得出重发的造会话，会交回上一次那一个。
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let script = Script::new([Play::Says("好")]);
    let mut client = Client::connect(home.core_configured(&script, None, &[]));
    client.hello().await;
    let created = client
        .call(
            &format!("create-{n}"),
            "session.create",
            json!({"cwd": cwd.to_string_lossy()}),
        )
        .await;
    let session = created["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    let said = client.say(&format!("send-{n}"), &session, "hi").await;
    let read_only = match &home.log(&session)[0].body {
        Body::SessionCreated(created) => created.permission.read_only,
        other => panic!("第一条是 session.created：{other:?}"),
    };
    (created, said, read_only)
}

#[tokio::test]
async fn an_unknown_project_config_does_not_count_and_is_pointed_out() {
    let home = Home::new();
    let repo = repo(&home, "app", STRICT);
    let deep = repo.join("src");
    std::fs::create_dir_all(&deep).expect("建得了");
    let (created, said, read_only) = open(&home, &deep).await;
    let file = repo.join(".miyu").join("config.toml");
    let shown = file
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    assert_eq!(
        created["result"]["untrusted_project"],
        json!(shown),
        "{created}"
    );
    assert_eq!(said["result"]["untrusted_project"], json!(shown), "{said}");
    assert!(!read_only, "没信任的不算");
}

#[tokio::test]
async fn a_trusted_one_counts_until_it_changes() {
    let home = Home::new();
    let repo = repo(&home, "app", STRICT);
    record(&home, &repo, STRICT, true);
    let (created, said, read_only) = open(&home, &repo).await;
    assert!(
        created["result"].get("untrusted_project").is_none(),
        "{created}"
    );
    assert!(said["result"].get("untrusted_project").is_none(), "{said}");
    assert!(read_only, "信任着、版本一样的算");
    std::fs::write(
        repo.join(".miyu").join("config.toml"),
        format!("{STRICT}\n"),
    )
    .expect("写得进");
    let (created, _, read_only) = open(&home, &repo).await;
    assert!(
        created["result"]["untrusted_project"].is_string(),
        "内容变了再问"
    );
    assert!(!read_only);
}

#[tokio::test]
async fn a_distrusted_one_does_not_count_and_is_not_pointed_out_again() {
    let home = Home::new();
    let repo = repo(&home, "app", STRICT);
    record(&home, &repo, STRICT, false);
    let (created, said, read_only) = open(&home, &repo).await;
    assert!(created["result"].get("untrusted_project").is_none());
    assert!(said["result"].get("untrusted_project").is_none());
    assert!(!read_only);
}

#[tokio::test]
async fn a_moved_repository_is_asked_again() {
    let home = Home::new();
    let old = repo(&home, "old", STRICT);
    record(&home, &old, STRICT, true);
    let moved = repo(&home, "moved", STRICT);
    let (created, _, read_only) = open(&home, &moved).await;
    assert!(created["result"]["untrusted_project"].is_string());
    assert!(!read_only);
}

#[tokio::test]
async fn get_with_a_directory_shows_the_project_and_its_trust() {
    let home = Home::new();
    let loose = "[permission]\nstart_read_only = false\n";
    let repo = repo(&home, "app", loose);
    home.write("system/config.toml", "permission.start_read_only = true\n");
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let cwd = repo.to_string_lossy().into_owned();
    let reply = client
        .call("c1", "config.get", json!({"cwd": cwd, "all": true}))
        .await;
    let project = &reply["result"]["files"]["project"];
    assert_eq!(project["trusted"], Value::Null, "还没问过");
    assert_eq!(project["version"], json!(version(loose.as_bytes())));
    let codes: Vec<&str> = reply["result"]["problems"]
        .as_array()
        .expect("是数组")
        .iter()
        .map(|p| p["code"].as_str().expect("有原因码"))
        .collect();
    assert_eq!(codes, ["untrusted_project"]);
    let layers = &reply["result"]["items"]["permission.start_read_only"]["layers"];
    assert_eq!(layers[0]["origin"]["layer"], "project");
    assert_eq!(
        (layers[0]["used"].clone(), layers[0]["problem"].clone()),
        (json!(false), json!("untrusted_project"))
    );

    record(&home, &repo, loose, true);
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let reply = client
        .call("c1", "config.get", json!({"cwd": cwd, "all": true}))
        .await;
    assert_eq!(reply["result"]["files"]["project"]["trusted"], true);
    assert_eq!(
        reply["result"]["items"]["permission.start_read_only"]["value"], true,
        "宽的不算"
    );
    let problem = &reply["result"]["problems"][0];
    assert_eq!(problem["code"], "not_tightening");
    assert_eq!(
        problem["message"],
        "项目配置只能让限制更严。permission.start_read_only 现在是 true，这里写的 false 更宽，不算。"
    );
    let layers = &reply["result"]["items"]["permission.start_read_only"]["layers"];
    assert_eq!(layers[0]["problem"], "not_tightening");
}

/// 照 `cwd` 信任（`trust`）版本是 `text` 的那一份。
async fn answer(client: &mut Client, id: &str, cwd: &Path, text: &str, trust: bool) -> Value {
    client
        .call(
            id,
            "config.trust",
            json!({"cwd": cwd.to_string_lossy(), "version": version(text.as_bytes()), "trust": trust}),
        )
        .await
}

#[tokio::test]
async fn trusting_records_the_answer_once_per_repository() {
    let home = Home::new();
    let repo = repo(&home, "app", STRICT);
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let reply = answer(
        &mut client,
        "config-9f2c4e1a7b3d5f60-2",
        &repo.join("src"),
        STRICT,
        true,
    )
    .await;
    let file = repo.join(".miyu").join("config.toml");
    let shown = file
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    assert_eq!(
        reply["result"],
        json!({"file": shown, "trusted": true}),
        "{reply}"
    );
    let trust = home.root.path().join("home/alice/trust.toml");
    let written = std::fs::read_to_string(&trust).expect("写了");
    assert!(
        written.starts_with(
            "# Miyu 记着的项目配置的信任：哪个仓库、哪一份内容、信不信任。\n[[project]]\n"
        ),
        "{written}"
    );
    let (created, _, read_only) = open(&home, &repo).await;
    assert!(created["result"].get("untrusted_project").is_none());
    assert!(read_only, "信任了就算");

    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    answer(&mut client, "c2", &repo, STRICT, false).await;
    let again = std::fs::read_to_string(&trust).expect("在");
    assert_eq!(
        again.matches("[[project]]").count(),
        1,
        "一个仓库一条：{again}"
    );
    assert!(again.contains("trusted = false"));
    let (created, _, read_only) = open(&home, &repo).await;
    assert!(
        created["result"].get("untrusted_project").is_none(),
        "不信任的不再提醒"
    );
    assert!(!read_only);

    let logged: Vec<Value> =
        std::fs::read_to_string(home.root.path().join("home/alice/journal.jsonl"))
            .expect("记了")
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON"))
            .collect();
    assert_eq!(logged.len(), 2);
    assert_eq!(logged[0]["kind"], "trust.changed");
    assert_eq!(logged[0]["cause"], "config-9f2c4e1a7b3d5f60-2");
    assert_eq!(
        logged[0]["body"]["version"],
        json!(version(STRICT.as_bytes()))
    );
    assert_eq!(logged[0]["body"]["trusted"], true);
    assert_eq!(logged[1]["seq"], 2);
    assert_eq!(logged[1]["body"]["trusted"], false);
}

#[tokio::test]
async fn the_answer_must_be_about_the_version_that_was_looked_at() {
    let home = Home::new();
    let repo = repo(&home, "app", STRICT);
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let stale = answer(&mut client, "c1", &repo, "[permission]\n", true).await;
    assert_eq!(reason(&stale), Some("config_conflict"));
    assert_eq!(
        stale["error"]["data"]["version"],
        json!(version(STRICT.as_bytes()))
    );
    let nowhere = answer(&mut client, "c2", &home.work, STRICT, true).await;
    assert_eq!(reason(&nowhere), Some("no_project_config"));
    assert_eq!(nowhere["error"]["message"], "这个目录找不到项目配置。");
    let missing = client
        .call(
            "c3",
            "config.trust",
            json!({"cwd": repo.to_string_lossy(), "trust": true}),
        )
        .await;
    assert_eq!(reason(&missing), Some("bad_params"));
    assert!(
        !home.root.path().join("home/alice/trust.toml").exists(),
        "什么都没记"
    );
}

#[tokio::test]
async fn a_hand_edited_record_file_is_kept() {
    let home = Home::new();
    let other = repo(&home, "other", STRICT);
    let app = repo(&home, "app", STRICT);
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    // 核心起来以后手写一条：信任 other。
    record(&home, &other, STRICT, true);
    let hand = std::fs::read_to_string(home.root.path().join("home/alice/trust.toml")).expect("在");
    answer(&mut client, "c1", &app, STRICT, true).await;
    let written =
        std::fs::read_to_string(home.root.path().join("home/alice/trust.toml")).expect("在");
    assert!(written.starts_with(&hand), "手写的一字不动：{written}");
    let got = client
        .call("c2", "config.get", json!({"cwd": other.to_string_lossy()}))
        .await;
    assert_eq!(
        got["result"]["files"]["project"]["trusted"], true,
        "同一个核心换上了连手写的在内的记录"
    );
}
