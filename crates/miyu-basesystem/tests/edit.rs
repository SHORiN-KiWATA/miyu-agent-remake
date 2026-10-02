//! `edit`（施工 4-6 中）：参数的几种写法；几处一起改、对照的是原文件；重叠的、有一处出错的一处都不改；CRLF、BOM、
//! UTF-16 照原来的；宽松对上只换那一段；不改的几种；没对上给最接近的几行，不唯一给行号；`replace_all`；效果和说法。

mod support;

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::Said;
use miyu_kernel::id::ContentHash;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Seen};

use support::{Site, native, tool};

/// 基础系统的说法。
fn said(key: &str) -> Said {
    Said::new(format!("software/basesystem/{key}"))
}

/// 交回的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 结果里的路径：相对工作目录、照本平台的分隔符，照模板转义过、带引号。
fn quoted(path: &str) -> String {
    serde_json::to_string(&native(path)).expect("字写得成 JSON")
}

/// 场地里 `work/` 下写一个文件，她读过了：交回她看过的。
fn seen_file(site: &Site, name: &str, bytes: &[u8]) -> Seen {
    let path = format!("work/{name}");
    site.file(&path, bytes);
    Seen::from([(site.real(&path), ContentHash::of(bytes))])
}

/// 场地里 `work/` 下那个文件现在的字节。
fn bytes(site: &Site, name: &str) -> Vec<u8> {
    std::fs::read(site.0.join("work").join(name)).expect("读得出")
}

/// 在 `work/` 里调一次 `edit`，参数是 `args`，她看过的是 `seen`。
async fn edit(site: &Site, args: Value, seen: Seen) -> Done {
    site.done_seen("work", "edit", args, seen).await
}

#[test]
fn edit_comes_from_the_resources_and_names_what_it_writes() {
    let tool = tool("edit");
    assert_eq!(tool.spec().access, Access::Write);
    let targets = tool.targets(&Call {
        args: json!({"file_path": "a.rs", "edits": []}).to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
    });
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].path, "a.rs");
    assert!(targets[0].write);
}

#[tokio::test]
async fn several_edits_are_matched_against_the_original() {
    let site = Site::new();
    let seen = seen_file(&site, "a.txt", b"one\ntwo\nthree\n");
    let done = edit(
        &site,
        json!({"file_path": "a.txt", "edits": [
            {"old_string": "three", "new_string": "3"},
            {"old_string": "one", "new_string": "one\ntwo"},
            {"old_string": "two\n", "new_string": ""},
        ]}),
        seen,
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        bytes(&site, "a.txt"),
        b"one\ntwo\n3\n",
        "每一处对照原文件找"
    );
    assert_eq!(text(&done), format!("Edited {}.\n", quoted("a.txt")));
    assert_eq!(done.human, Some(said("edit/edited").with("count", "3")));
    assert_eq!(
        done.effects,
        [Effect::Changed {
            path: site.real("work/a.txt"),
            before: Some(b"one\ntwo\nthree\n".to_vec()),
            after: b"one\ntwo\n3\n".to_vec(),
        }]
    );
    // 前面一处变长了：后面那一处照样换在原来的位置上。
    let seen = seen_file(&site, "b.txt", b"a-b-c\n");
    let done = edit(
        &site,
        json!({"file_path": "b.txt", "edits": [
            {"old_string": "a", "new_string": "AAAA"},
            {"old_string": "c", "new_string": "C"},
        ]}),
        seen,
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(bytes(&site, "b.txt"), b"AAAA-b-C\n");
}

#[tokio::test]
async fn the_claude_code_shape_and_a_single_object_are_understood() {
    let site = Site::new();
    let seen = seen_file(&site, "a.txt", b"x = 1\n");
    let done = edit(
        &site,
        json!({"file_path": "a.txt", "old_string": "1", "new_string": "2"}),
        seen,
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(bytes(&site, "a.txt"), b"x = 2\n");
    let seen = Seen::from([(site.real("work/a.txt"), ContentHash::of(b"x = 2\n"))]);
    let done = edit(
        &site,
        json!({"path": "a.txt", "edits": {"oldString": "2", "newString": "3"}}),
        seen.clone(),
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(bytes(&site, "a.txt"), b"x = 3\n");
    let done = edit(&site, json!({"file_path": "a.txt"}), seen).await;
    assert!(done.error);
    assert_eq!(done.human, Some(said("edit/no-edits")));
}

#[tokio::test]
async fn overlapping_or_failing_edits_change_nothing() {
    let site = Site::new();
    let seen = seen_file(&site, "a.txt", b"alpha beta gamma\n");
    let done = edit(
        &site,
        json!({"file_path": "a.txt", "edits": [
            {"old_string": "beta gamma", "new_string": "b g"},
            {"old_string": "alpha beta", "new_string": "a b"},
        ]}),
        seen.clone(),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        format!(
            "Edits 1 and 2 overlap in {}. Merge them into one edit.\n",
            quoted("a.txt")
        )
    );
    assert_eq!(
        done.human,
        Some(said("edit/overlap").with("first", "1").with("second", "2"))
    );
    let done = edit(
        &site,
        json!({"file_path": "a.txt", "edits": [
            {"old_string": "alpha", "new_string": "a"},
            {"old_string": "delta", "new_string": "d"},
        ]}),
        seen,
    )
    .await;
    assert!(done.error);
    assert_eq!(done.human, Some(said("edit/not-found").with("index", "2")));
    assert_eq!(bytes(&site, "a.txt"), b"alpha beta gamma\n", "一处都没改");
}

#[tokio::test]
async fn line_endings_bom_and_encoding_stay_as_they_were() {
    let site = Site::new();
    // CRLF 的文件：她照读到的 LF 写，新加的行是 CRLF；别的地方混着的换行一个不动。
    let crlf = b"a\r\nb\r\nc\nd\r\n";
    let seen = seen_file(&site, "crlf.txt", crlf);
    let done = edit(
        &site,
        json!({"file_path": "crlf.txt", "old_string": "b\n", "new_string": "b\nb2\n"}),
        seen,
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(bytes(&site, "crlf.txt"), b"a\r\nb\r\nb2\r\nc\nd\r\n");
    for (name, old, want) in [
        (
            "bom.txt",
            &b"\xEF\xBB\xBFx = 1\n"[..],
            &b"\xEF\xBB\xBFx = 2\n"[..],
        ),
        ("utf16.txt", b"\xFF\xFEx\x001\x00", b"\xFF\xFEx\x002\x00"),
    ] {
        let seen = seen_file(&site, name, old);
        let done = edit(
            &site,
            json!({"file_path": name, "old_string": "1", "new_string": "2"}),
            seen,
        )
        .await;
        assert!(!done.error, "{name}: {}", text(&done));
        assert_eq!(bytes(&site, name), want, "{name}");
    }
}

#[tokio::test]
async fn a_loose_match_replaces_only_what_it_matched() {
    let site = Site::new();
    let old = "print(\u{201C}hi\u{201D})   \nkeep\u{00A0}this\n";
    let seen = seen_file(&site, "a.py", old.as_bytes());
    let done = edit(
        &site,
        json!({"file_path": "a.py", "old_string": "print(\"hi\")", "new_string": "print(\"hello\")"}),
        seen,
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        String::from_utf8(bytes(&site, "a.py")).expect("UTF-8"),
        "print(\"hello\")   \nkeep\u{00A0}this\n",
        "对上的那一段换了，行尾的空白、别处的不换行空格照旧"
    );
}

#[tokio::test]
async fn a_miss_shows_the_closest_lines_and_many_matches_show_where() {
    let site = Site::new();
    let content = b"fn main() {\n    let total = count(items);\n    show(total);\n}\n";
    let seen = seen_file(&site, "main.rs", content);
    let done = edit(
        &site,
        json!({"file_path": "main.rs", "old_string": "    let totals = count(item);", "new_string": "x"}),
        seen.clone(),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        format!(
            "Edit 1: old_string was not found in {}.\nThe closest text is at lines 2-2:\n2\t    let total = count(items);\n",
            quoted("main.rs")
        )
    );
    assert_eq!(
        done.human,
        Some(
            said("edit/not-found-near")
                .with("index", "1")
                .with("line", "2")
        )
    );
    let seen = seen_file(&site, "many.txt", b"x\ny\nx\n");
    let done = edit(
        &site,
        json!({"file_path": "many.txt", "old_string": "x", "new_string": "z"}),
        seen.clone(),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        format!(
            "Edit 1: old_string matches 2 places in {}, at lines 1, 3. Add surrounding lines to pick one, or set replace_all.\n",
            quoted("many.txt")
        )
    );
    assert_eq!(
        done.human,
        Some(
            said("edit/not-unique")
                .with("index", "1")
                .with("count", "2")
        )
    );
    // replace_all：每一处都换。
    let done = edit(
        &site,
        json!({"file_path": "many.txt", "old_string": "x", "new_string": "z", "replace_all": true}),
        seen,
    )
    .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(bytes(&site, "many.txt"), b"z\ny\nz\n");
    assert_eq!(done.human, Some(said("edit/edited").with("count", "2")));
}

#[tokio::test]
async fn what_is_not_edited() {
    let site = Site::new();
    // 没读过、读过以后被改了。
    site.file("work/a.txt", b"a\n");
    let args = json!({"file_path": "a.txt", "old_string": "a", "new_string": "b"});
    let done = edit(&site, args.clone(), Seen::new()).await;
    assert_eq!(done.human, Some(said("common/not-read")));
    let stale = Seen::from([(site.real("work/a.txt"), ContentHash::of(b"older\n"))]);
    let done = edit(&site, args, stale).await;
    assert_eq!(done.human, Some(said("common/stale")));
    // 不存在的，带上相近的名字；目录；不是文本。
    let done = edit(
        &site,
        json!({"file_path": "b.txt", "old_string": "a", "new_string": "b"}),
        Seen::new(),
    )
    .await;
    assert!(
        matches!(&done.human, Some(said) if said.key.ends_with("common/missing") || said.key.ends_with("common/missing-similar"))
    );
    std::fs::create_dir_all(site.0.join("work/dir")).expect("建得了");
    let done = edit(
        &site,
        json!({"file_path": "dir", "old_string": "a", "new_string": "b"}),
        Seen::new(),
    )
    .await;
    assert_eq!(done.human, Some(said("common/directory")));
    let seen = seen_file(&site, "bin", b"a\xFF\xFEb");
    let done = edit(
        &site,
        json!({"file_path": "bin", "old_string": "a", "new_string": "b"}),
        seen,
    )
    .await;
    assert_eq!(done.human, Some(said("edit/not-text")));
    // `old_string` 空的、改前改后一样的。
    let seen = seen_file(&site, "c.txt", b"c\n");
    let done = edit(
        &site,
        json!({"file_path": "c.txt", "old_string": "", "new_string": "x"}),
        seen.clone(),
    )
    .await;
    assert_eq!(done.human, Some(said("edit/empty").with("index", "1")));
    let done = edit(
        &site,
        json!({"file_path": "c.txt", "old_string": "c", "new_string": "c"}),
        seen,
    )
    .await;
    assert_eq!(done.human, Some(said("edit/same").with("index", "1")));
    assert_eq!(bytes(&site, "c.txt"), b"c\n");
}
