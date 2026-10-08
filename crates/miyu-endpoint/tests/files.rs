//! `fs.list`、`fs.find`（施工 W-2，`docs/blueprint/web-module.md`「三、列文件、找文件」、`protocol.md`
//! 「`fs.list`」「`fs.find`」）：真核心上数据根不列不找、账号的工作区照样列；清单没建完先给一部分、`building`；
//! `fresh` 隔一段时间才重建；最多记几份、多了丢最久没用的；`path` 一律用 `/`。

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 连上、握手（中文），交回客户端。
async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 建好账号的家目录、工作区（`session.create` 以前就要用到工作区时，测试自己先建）。交回工作区的真实位置。
fn workspace(home: &Home) -> std::path::PathBuf {
    home.root
        .prepare_home(&alice())
        .expect("建得了家目录、工作区");
    std::fs::canonicalize(home.root.workspace(&alice())).expect("工作区在")
}

fn touch(root: &Path, relative: &str) {
    let path = root.join(relative);
    if relative.ends_with('/') {
        std::fs::create_dir_all(&path).expect("建得了目录");
    } else {
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(&path, "x").expect("写得进");
    }
}

/// 回应里的 `items`，每一条拿出 `path`。
fn paths(reply: &Value) -> Vec<String> {
    reply["result"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| item["path"].as_str().unwrap_or("").to_string())
        .collect()
}

/// 一直发 `fs.find`，等到 `building` 是 `false`，最多六十秒：建清单在后台线程里，别写成死等。
async fn until_built(client: &mut Client, cwd: &str, query: &str, fresh: bool) -> Value {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        let reply = client
            .call(
                "f",
                "fs.find",
                json!({"cwd": cwd, "query": query, "fresh": fresh}),
            )
            .await;
        if reply["result"]["building"] == json!(false) {
            return reply;
        }
        assert!(Instant::now() < until, "六十秒内清单没建完：{reply}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
async fn names_are_filtered_by_prefix_dirs_first_marks_cover_the_prefix() {
    let home = Home::new();
    let work = workspace(&home);
    for p in ["b.txt", "a/", "Abc.md", ".hidden", "zeta/", "ab.rs"] {
        touch(&work, p);
    }
    let mut client = client(&home).await;

    let cwd = work.display().to_string();
    let all = client
        .call(
            "l1",
            "fs.list",
            json!({"cwd": cwd, "dir": "", "prefix": ""}),
        )
        .await;
    assert_eq!(
        paths(&all),
        ["a/", "zeta/", "ab.rs", "Abc.md", "b.txt"],
        "目录在前，各照名字排"
    );
    assert_eq!(all["result"]["partial"], json!(false));
    let first = &all["result"]["items"][2];
    assert_eq!(first["path"], json!("ab.rs"));
    assert_eq!(first["dir"], json!(false));
    assert_eq!(first["size"], json!(1));
    assert!(Path::new(first["full"].as_str().expect("full")).is_absolute());

    let a = client
        .call(
            "l2",
            "fs.list",
            json!({"cwd": cwd, "dir": "", "prefix": "a"}),
        )
        .await;
    assert_eq!(paths(&a), ["a/", "ab.rs", "Abc.md"], "开头对、大小写不论");
    assert_eq!(
        a["result"]["items"][0]["marks"],
        json!([0]),
        "prefix 几个字，marks 就几个"
    );

    let dot = client
        .call(
            "l3",
            "fs.list",
            json!({"cwd": cwd, "dir": "", "prefix": "."}),
        )
        .await;
    assert_eq!(paths(&dot), [".hidden"], "打了点才列点开头的");
}

#[tokio::test]
async fn fs_list_rejects_the_data_root_and_lets_the_workspace_through() {
    let home = Home::new();
    let work = workspace(&home);
    let mut client = client(&home).await;

    let root_reply = client
        .call(
            "l1",
            "fs.list",
            json!({"cwd": home.root.path().display().to_string(), "dir": "", "prefix": ""}),
        )
        .await;
    assert_eq!(reason(&root_reply), Some("path_forbidden"), "{root_reply}");

    let work_reply = client
        .call(
            "l2",
            "fs.list",
            json!({"cwd": work.display().to_string(), "dir": "", "prefix": ""}),
        )
        .await;
    assert!(
        work_reply.get("error").is_none(),
        "工作区照样能列：{work_reply}"
    );

    // `dir` 往上走出工作区、落回数据根里的也一样拒。
    let via_dir = client
        .call(
            "l3",
            "fs.list",
            json!({"cwd": work.display().to_string(), "dir": "..", "prefix": ""}),
        )
        .await;
    assert_eq!(reason(&via_dir), Some("path_forbidden"), "{via_dir}");
}

#[tokio::test]
async fn an_unresolvable_or_non_directory_cwd_is_path_unreadable() {
    let home = Home::new();
    let work = workspace(&home);
    touch(&work, "f.txt");
    let mut client = client(&home).await;

    let missing = client
        .call(
            "l1",
            "fs.list",
            json!({"cwd": work.join("does-not-exist").display().to_string(), "dir": "", "prefix": ""}),
        )
        .await;
    assert_eq!(reason(&missing), Some("path_unreadable"), "{missing}");

    let a_file = client
        .call(
            "l2",
            "fs.list",
            json!({"cwd": work.join("f.txt").display().to_string(), "dir": "", "prefix": ""}),
        )
        .await;
    assert_eq!(
        reason(&a_file),
        Some("path_unreadable"),
        "该是目录的不是：{a_file}"
    );

    // `fs.find` 没有 `fs.list` 那样走目录时顺带失败的安全网：`cwd` 该是目录的不是，自己要拦住。
    let find_a_file = client
        .call(
            "f1",
            "fs.find",
            json!({"cwd": work.join("f.txt").display().to_string(), "query": ""}),
        )
        .await;
    assert_eq!(
        reason(&find_a_file),
        Some("path_unreadable"),
        "fs.find 的 cwd 该是目录的不是：{find_a_file}"
    );
}

#[tokio::test]
async fn fs_list_without_a_cwd_is_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = client.call("l1", "fs.list", json!({})).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn fs_find_matches_fuzzy_queries_with_marks_and_nested_paths_use_forward_slashes() {
    let home = Home::new();
    let work = workspace(&home);
    touch(&work, "src/main.rs");
    touch(&work, "src/lib.rs");
    touch(&work, "docs/readme.md");
    let mut client = client(&home).await;

    let cwd = work.display().to_string();
    let reply = until_built(&mut client, &cwd, "main", false).await;
    assert_eq!(paths(&reply), ["src/main.rs"], "{reply}");
    assert_eq!(reply["result"]["items"][0]["marks"], json!([4, 5, 6, 7]));
    assert_eq!(reply["result"]["partial"], json!(false));

    let all = until_built(&mut client, &cwd, "", false).await;
    let mut got = paths(&all);
    got.sort();
    assert_eq!(
        got,
        [
            "docs/",
            "docs/readme.md",
            "src/",
            "src/lib.rs",
            "src/main.rs"
        ],
        "路径用 /，含子目录；目录自己也收，后面带 /"
    );
}

#[tokio::test]
async fn fs_find_orders_higher_scores_first_without_the_client_resorting() {
    let home = Home::new();
    let work = workspace(&home);
    // 都是平级文件，省得目录自己也对上 "main" 添乱。去掉扩展名正好是 "main" 的分最高，字连着的次之，
    // 字隔开了的分最低。
    touch(&work, "main.rs");
    touch(&work, "amainb.rs");
    touch(&work, "m1a2i3n4.rs");
    let mut client = client(&home).await;

    let reply = until_built(&mut client, &work.display().to_string(), "main", false).await;
    assert_eq!(
        paths(&reply),
        ["main.rs", "amainb.rs", "m1a2i3n4.rs"],
        "核心自己排过序，不是客户端排的：{reply}"
    );
}

#[tokio::test]
async fn fs_find_rejects_the_data_root_and_lets_the_workspace_through() {
    let home = Home::new();
    let work = workspace(&home);
    let mut client = client(&home).await;

    let root_reply = client
        .call(
            "f1",
            "fs.find",
            json!({"cwd": home.root.path().display().to_string(), "query": ""}),
        )
        .await;
    assert_eq!(reason(&root_reply), Some("path_forbidden"), "{root_reply}");

    let work_reply = until_built(&mut client, &work.display().to_string(), "", false).await;
    assert!(
        work_reply.get("error").is_none(),
        "工作区照样能找：{work_reply}"
    );
}

#[tokio::test]
async fn fresh_only_rebuilds_after_the_configured_duration() {
    let home = Home::new();
    let work = workspace(&home);
    touch(&work, "one.txt");
    let fresh_after = Duration::from_millis(80);
    let core = home.core_files_fresh(&Script::new([]), fresh_after);
    let mut client = Client::connect(core);
    client.hello().await;

    let cwd = work.display().to_string();
    let first = until_built(&mut client, &cwd, "", true).await;
    assert_eq!(paths(&first), ["one.txt"]);

    touch(&work, "two.txt");
    let still_stale = client
        .call(
            "f1",
            "fs.find",
            json!({"cwd": cwd, "query": "", "fresh": true}),
        )
        .await;
    assert!(
        !paths(&still_stale).contains(&"two.txt".to_string()),
        "还没到 fresh_after，不重建：{still_stale}"
    );

    tokio::time::sleep(fresh_after + Duration::from_millis(40)).await;
    let rebuilt = until_built(&mut client, &cwd, "", true).await;
    assert!(
        paths(&rebuilt).contains(&"two.txt".to_string()),
        "过了 fresh_after、又写了 fresh:true，该重建了：{rebuilt}"
    );

    // `fresh` 不是 `true` 的，隔多久都不重建。
    touch(&work, "three.txt");
    let not_fresh = client
        .call(
            "f2",
            "fs.find",
            json!({"cwd": cwd, "query": "", "fresh": false}),
        )
        .await;
    assert!(
        !paths(&not_fresh).contains(&"three.txt".to_string()),
        "fresh 不是 true，不重建：{not_fresh}"
    );
}

#[tokio::test]
async fn at_most_four_directories_are_cached_the_oldest_unused_one_is_dropped() {
    let home = Home::new();
    let work = workspace(&home);
    let dirs: Vec<std::path::PathBuf> = ('a'..='e')
        .map(|letter| {
            let dir = work.join(letter.to_string());
            touch(&dir, "f.txt");
            dir
        })
        .collect();
    let mut client = client(&home).await;

    // 先问 a、b、c、d：缓存刚好记满 4 份。
    for dir in &dirs[0..4] {
        let reply = until_built(&mut client, &dir.display().to_string(), "", false).await;
        assert_eq!(paths(&reply), ["f.txt"], "{reply}");
    }
    // 再问 e：腾地方丢最久没用的（a）。
    let e_reply = until_built(&mut client, &dirs[4].display().to_string(), "", false).await;
    assert_eq!(paths(&e_reply), ["f.txt"], "{e_reply}");

    // a 现在该是新起一份清单了：在磁盘上加一个文件，`fresh:false` 也照样照新的来（证明不是复用旧的那份）。
    touch(&dirs[0], "new.txt");
    let a_again = until_built(&mut client, &dirs[0].display().to_string(), "", false).await;
    assert!(
        paths(&a_again).contains(&"new.txt".to_string()),
        "a 被丢了、重新建的清单该看得到新文件：{a_again}"
    );

    // b 还在缓存里：新加的文件在 `fresh:false` 时看不到。
    touch(&dirs[1], "new.txt");
    let b_again = client
        .call(
            "f1",
            "fs.find",
            json!({"cwd": dirs[1].display().to_string(), "query": "", "fresh": false}),
        )
        .await;
    assert!(
        !paths(&b_again).contains(&"new.txt".to_string()),
        "b 没被丢，复用旧的那份：{b_again}"
    );
}
