//! 系统日志、账号日志：`seq` 从 1 数起、接着数；打开时截掉最后那半行；一行一条，外壳照事件的写法、没有 `turn`；最后一行
//! 手改坏了的不往后写。

use std::fs;

use miyu_kernel::id::{AccountId, CommandId, EventKind};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;

use super::*;
use crate::test_support::Scratch;

/// 一条：第 `n` 次改。
fn entry(n: u32) -> Entry {
    Entry {
        at: Timestamp::parse("2026-10-01T08:00:00.000Z").unwrap(),
        by: By::Person(Person::new(AccountId::parse("admin").unwrap())),
        cause: Some(CommandId::parse(&format!("config-9f2c4e1a7b3d5f60-{n}")).unwrap()),
        kind: EventKind::parse("config.changed").unwrap(),
        body: serde_json::from_str::<RawJson>(&format!("{{\"n\":{n}}}")).unwrap(),
    }
}

fn lines(path: &std::path::Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn entries_are_counted_from_one_and_written_like_events() {
    let temp = Scratch::new();
    let path = temp.path().join("home").join("admin").join(FILE);
    assert_eq!(append(&path, entry(1)).unwrap().get(), 1);
    assert_eq!(append(&path, entry(2)).unwrap().get(), 2);
    assert_eq!(
        lines(&path),
        [
            "{\"seq\":1,\"at\":\"2026-10-01T08:00:00.000Z\",\"kind\":\"config.changed\",\"by\":{\"kind\":\"person\",\"account\":\"admin\"},\"cause\":\"config-9f2c4e1a7b3d5f60-1\",\"body\":{\"n\":1}}",
            "{\"seq\":2,\"at\":\"2026-10-01T08:00:00.000Z\",\"kind\":\"config.changed\",\"by\":{\"kind\":\"person\",\"account\":\"admin\"},\"cause\":\"config-9f2c4e1a7b3d5f60-2\",\"body\":{\"n\":2}}",
        ]
    );
    let seen = Event::from_line(&lines(&path)[0]).unwrap();
    assert!(
        matches!(seen.body, Body::Unknown { .. }),
        "内核照不认识的种类留着"
    );
}

#[test]
fn a_half_written_last_line_is_cut_and_the_count_goes_on() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    append(&path, entry(1)).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"{\"seq\":2,\"at\":");
    fs::write(&path, &bytes).unwrap();
    assert_eq!(append(&path, entry(3)).unwrap().get(), 2, "半行不算");
    let lines = lines(&path);
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1],
        "{\"seq\":2,\"at\":\"2026-10-01T08:00:00.000Z\",\"kind\":\"config.changed\",\"by\":{\"kind\":\"person\",\"account\":\"admin\"},\"cause\":\"config-9f2c4e1a7b3d5f60-3\",\"body\":{\"n\":3}}",
        "半行截掉了，新的一行完整"
    );
}

#[test]
fn someone_elses_entries_are_counted_on_from() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join(FILE);
    fs::write(
        &path,
        "{\"seq\":7,\"at\":\"2026-10-01T08:00:00.000Z\",\"kind\":\"trust.changed\",\"by\":{\"kind\":\"kernel\"},\"body\":{}}\n",
    )
    .unwrap();
    assert_eq!(append(&path, entry(1)).unwrap().get(), 8);
}

#[test]
fn a_broken_last_line_stops_the_write() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join(FILE);
    fs::write(&path, "not json\n").unwrap();
    let error = append(&path, entry(1)).unwrap_err();
    assert!(error.to_string().contains("line 1"), "{error}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "not json\n", "没往后写");
}
