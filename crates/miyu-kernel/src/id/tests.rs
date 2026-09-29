//! 编号和名字的测试：图纸上的例子读写一字不差；每一条规则各有一个坏例子，证明读的时候拦得下。

use super::*;
use crate::test_support::{rejected, round_trip};

#[test]
fn samples_from_the_drawing_round_trip() {
    round_trip::<SessionId>(r#""0192f3a0-1111-7abc-8def-001122334455""#);
    round_trip::<Seq>("45");
    round_trip::<TurnId>("42");
    round_trip::<CommandId>(r#""cmd-7f3a""#);
    round_trip::<CallId>(r#""call_44_1""#);
    round_trip::<JobId>(r#""j2""#);
    round_trip::<JobId>(r#""j2.1""#);
    round_trip::<JobId>(r#""j2.1.1""#);
    round_trip::<AccountId>(r#""alice""#);
    round_trip::<ContentHash>(
        r#""sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855""#,
    );
}

#[test]
fn session_id_must_be_lowercase_uuid_text() {
    rejected::<SessionId>(r#""0192F3A0-1111-7abc-8def-001122334455""#, "lowercase hex");
    rejected::<SessionId>(r#""0192f3a0-1111-7abc-8def-00112233445""#, "36 characters");
    rejected::<SessionId>(r#""0192f3a0_1111-7abc-8def-001122334455""#, "must be -");
}

#[test]
fn command_id_is_short_printable_text() {
    rejected::<CommandId>(r#""""#, "must not be empty");
    rejected::<CommandId>(&format!("\"{}\"", "x".repeat(129)), "128 bytes");
    rejected::<CommandId>(r#""cmd\n1""#, "control characters");
    round_trip::<CommandId>(&format!("\"{}\"", "x".repeat(128)));
}

#[test]
fn call_id_accepts_only_what_the_kernel_writes() {
    for bad in [
        "call_044_1",
        "call_44_01",
        "call_0_1",
        "call_44_0",
        "call_+4_1",
        "call_44",
        "cal_44_1",
        "call_44_1_2",
        "call_44_4294967296",
    ] {
        assert!(CallId::parse(bad).is_err(), "{bad} 不该读得进来");
    }
    let call = CallId::new(Seq::new(44).unwrap(), 1).unwrap();
    assert_eq!(call.to_string(), "call_44_1");
    assert_eq!(CallId::parse("call_44_1"), Ok(call));
    assert_eq!(call.message().get(), 44);
    assert_eq!(call.index(), 1);
}

/// 任务编号（施工 7-1，`kernel/ids.md` 第 25、26 条）：只认 `j` 加一段或几段从 1 起的十进制数、段之间用 `.` 连，内核
/// 自己写出去的样子（几段的，施工 7-1 补）。
#[test]
fn job_id_accepts_only_what_the_kernel_writes() {
    const WHY: &str = "decimal numbers from 1 after j, joined by dots";
    for (bad, why) in [
        ("1", "must start with j"),
        ("J1", "must start with j"),
        ("J1.1", "must start with j"),
        ("", "must start with j"),
        (".j1", "must start with j"),
        ("job1", WHY),
        ("j", WHY),
        ("j0", WHY),
        ("j01", WHY),
        ("j+1", WHY),
        ("j1x", WHY),
        ("j 1", WHY),
        ("j18446744073709551616", WHY),
        ("j1.0", WHY),
        ("j0.1", WHY),
        ("j1.", WHY),
        ("j.1", WHY),
        ("j.", WHY),
        ("j1..2", WHY),
        ("j1.01", WHY),
        ("j1.x", WHY),
        ("j1.+2", WHY),
        ("j1.2.0", WHY),
        ("j1,2", WHY),
        ("j1.18446744073709551616", WHY),
    ] {
        rejected::<JobId>(&format!("\"{bad}\""), why);
        let err = JobId::parse(bad).unwrap_err();
        assert_eq!(err.what, "job id", "{bad}");
    }
    rejected::<JobId>("1", "invalid type");
    let job = JobId::new(12).unwrap();
    assert_eq!(job.to_string(), "j12");
    assert_eq!(JobId::parse("j12"), Ok(job.clone()));
    assert_eq!(job.last(), 12);
    assert_eq!(JobId::new(0), None);
    round_trip::<JobId>(r#""j1""#);
    round_trip::<JobId>(r#""j18446744073709551615""#);
    round_trip::<JobId>(r#""j2.1""#);
    round_trip::<JobId>(r#""j18446744073709551615.18446744073709551615""#);
    round_trip::<JobId>(r#""j3.12.1.7""#);
}

/// 子会话派的接在它自己的编号后面（施工 7-1 补）：`j2` 下面第 1 个是 `j2.1`，`j2.1` 下面第 1 个是 `j2.1.1`；最后一段是在
/// 这个会话里数的那个数。
#[test]
fn a_job_under_another_adds_one_part() {
    let child = JobId::new(2).unwrap();
    let grandchild = child.under(1).unwrap();
    assert_eq!(grandchild.to_string(), "j2.1");
    assert_eq!(grandchild.last(), 1);
    assert_eq!(JobId::parse("j2.1"), Ok(grandchild.clone()));
    let command = grandchild.under(3).unwrap();
    assert_eq!(command.to_string(), "j2.1.3");
    assert_eq!(command.last(), 3);
    assert_eq!(child.under(0), None, "从 1 数起");
    assert_eq!(child.to_string(), "j2", "接一段不动原来的");
    assert_ne!(JobId::parse("j2.1").unwrap(), JobId::parse("j2").unwrap());
    assert_ne!(JobId::parse("j2.1").unwrap(), JobId::parse("j1").unwrap());
}

/// 排序一段一段照数比，不照字符串：`j2` 在 `j10` 前面；前面的段一样的，段少的在前（施工 7-1 补）。
#[test]
fn job_ids_sort_by_number() {
    let parse = |text| JobId::parse(text).unwrap();
    assert!(parse("j2") < parse("j10"));
    assert!(parse("j9") < parse("j10"));
    assert!(parse("j2") < parse("j2.1"));
    assert!(parse("j2.1") < parse("j2.2"));
    assert!(parse("j2.9") < parse("j2.10"));
    assert!(parse("j2.9") < parse("j10"));
    assert!(parse("j2.1.5") < parse("j2.2"));
    assert!(parse("j5.8") < parse("j7"));
}

#[test]
fn account_is_like_a_linux_login_name() {
    let longest = "a".repeat(32);
    for good in ["a", "alice", "bob_2-x", longest.as_str()] {
        assert!(AccountId::parse(good).is_ok(), "{good} 应该读得进来");
    }
    rejected::<AccountId>(r#""Alice""#, "start with a lowercase letter");
    rejected::<AccountId>(r#""1abc""#, "start with a lowercase letter");
    rejected::<AccountId>(r#""小明""#, "start with a lowercase letter");
    rejected::<AccountId>(r#""a b""#, "only lowercase letters");
    rejected::<AccountId>(r#""aB""#, "only lowercase letters");
    rejected::<AccountId>(&format!("\"{}\"", "a".repeat(33)), "at most 32");
    rejected::<AccountId>(r#""con""#, "reserved name on Windows");
    rejected::<AccountId>(r#""lpt9""#, "reserved name on Windows");
}

/// 算出来的内容哈希，和 SHA-256 公开的测试值一样（FIPS 180-2 的 "abc"，和空的内容）。
#[test]
fn content_hash_of_known_contents() {
    assert_eq!(
        ContentHash::of(b"abc").as_str(),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        ContentHash::of(b"").as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    // 文件名用的那一截：去掉 sha256:，只剩 64 位。
    assert_eq!(
        ContentHash::of(b"").hex(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn content_hash_is_sha256_in_lowercase_hex() {
    let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    rejected::<ContentHash>(&format!("\"SHA256:{hex}\""), "start with sha256:");
    rejected::<ContentHash>(
        &format!("\"sha256:{}\"", hex.to_uppercase()),
        "lowercase hex",
    );
    rejected::<ContentHash>(&format!("\"sha256:{}\"", &hex[1..]), "64 digits");
}

#[test]
fn seq_starts_at_one() {
    rejected::<Seq>("0", "starts at 1");
    for bad in ["-1", "1.0", r#""1""#] {
        assert!(
            serde_json::from_str::<Seq>(bad).is_err(),
            "{bad} 不该读得进来"
        );
    }
    assert_eq!(Seq::new(0), None);
    assert_eq!(Seq::FIRST.next().get(), 2);
}

#[test]
fn turn_id_reads_like_a_seq() {
    let turn: TurnId = serde_json::from_str("42").unwrap();
    assert_eq!(turn.started(), Seq::new(42).unwrap());
    rejected::<TurnId>("0", "starts at 1");
}

#[test]
fn error_says_what_why_and_what_was_read() {
    let err = SessionId::parse("x").unwrap_err();
    assert_eq!(
        err.to_string(),
        "bad session id: must be 36 characters (got \"x\")"
    );
}

#[test]
fn long_text_in_errors_is_cut() {
    let err = CommandId::parse(&"x".repeat(200)).unwrap_err();
    assert_eq!(err.text, format!("{}…", "x".repeat(80)));
}

#[test]
fn names_from_the_drawing_round_trip() {
    round_trip::<ModuleId>(r#""memory""#);
    round_trip::<DriverFamily>(r#""openai-chat""#);
    round_trip::<VenueId>(r#""qq:group:123456""#);
    round_trip::<ExternalId>(r#""qq:10086""#);
    round_trip::<ProviderId>(r#""deepseek""#);
    round_trip::<ModelName>(r#""deepseek-v4""#);
    round_trip::<MediaType>(r#""image/png""#);
    round_trip::<FileName>(r#""报告.pdf""#);
    round_trip::<FactKind>(r#""env""#);
}

#[test]
fn module_driver_and_fact_names_follow_the_account_rule() {
    rejected::<ModuleId>(r#""Memory""#, "start with a lowercase letter");
    rejected::<DriverFamily>(r#""openai.chat""#, "only lowercase letters");
    rejected::<ModuleId>(r#""nul""#, "reserved name on Windows");
    rejected::<FactKind>(r#""Env""#, "start with a lowercase letter");
}

#[test]
fn short_names_are_opaque_but_bounded() {
    round_trip::<ModelName>(r#""qwen/qwen3-235b-a22b@2026-07""#);
    rejected::<ProviderId>(r#""""#, "must not be empty");
    rejected::<VenueId>(&format!("\"{}\"", "v".repeat(129)), "128 bytes");
    rejected::<ExternalId>(r#""qq:\u000710086""#, "control characters");
    // 别的 harness 报的名字（施工 7-1）：照 external 的做法，只管长度和控制字符。
    round_trip::<HarnessName>(r#""claude-code""#);
    round_trip::<HarnessName>(r#""Codex CLI 0.9 / 工作站""#);
    rejected::<HarnessName>(r#""""#, "must not be empty");
    rejected::<HarnessName>(&format!("\"{}\"", "h".repeat(129)), "128 bytes");
    rejected::<HarnessName>(r#""claude\ncode""#, "control characters");
    rejected::<HarnessName>(r#""claude\u001b[31mcode""#, "control characters");
    assert_eq!(HarnessName::parse("").unwrap_err().what, "harness name");
}

#[test]
fn media_type_is_lowercase_type_slash_subtype() {
    round_trip::<MediaType>(
        r#""application/vnd.openxmlformats-officedocument.wordprocessingml.document""#,
    );
    rejected::<MediaType>(r#""image""#, "type/subtype");
    rejected::<MediaType>(r#""Image/PNG""#, "lowercase letters");
    rejected::<MediaType>(r#""image/png/x""#, "lowercase letters");
    rejected::<MediaType>(r#""image/""#, "lowercase letters");
}

#[test]
fn file_name_is_a_name_not_a_path() {
    rejected::<FileName>(r#""../etc/passwd""#, "/");
    rejected::<FileName>(r#""a\\b.txt""#, "/");
    rejected::<FileName>(r#""..""#, ". or ..");
    rejected::<FileName>(r#""""#, "must not be empty");
    rejected::<FileName>(&format!("\"{}\"", "报".repeat(86)), "255 bytes");
}

#[test]
fn event_kind_is_dotted_lowercase() {
    for good in [
        "message.user",
        "ext.memory.recalled",
        "ext.my-module.did_it2",
    ] {
        assert!(EventKind::parse(good).is_ok(), "{good} 应该读得进来");
    }
    rejected::<EventKind>(r#""""#, "must not be empty");
    rejected::<EventKind>(r#""message""#, "at least two parts");
    rejected::<EventKind>(r#""Message.user""#, "start with a lowercase letter");
    rejected::<EventKind>(r#"".user""#, "start with a lowercase letter");
    rejected::<EventKind>(r#""message..user""#, "start with a lowercase letter");
    rejected::<EventKind>(r#""message.9user""#, "start with a lowercase letter");
    rejected::<EventKind>(r#""message.us er""#, "only lowercase letters");
    rejected::<EventKind>(&format!("\"a.{}\"", "b".repeat(127)), "128 bytes");
}

#[test]
fn hashing_piece_by_piece_is_the_same_as_all_at_once() {
    let mut hasher = Hasher::default();
    for piece in [&b"hel"[..], b"", b"lo, world"] {
        hasher.update(piece);
    }
    assert_eq!(hasher.finish(), ContentHash::of(b"hello, world"));
    assert_eq!(
        Hasher::default().finish().to_string(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "什么都没喂的，是空内容的哈希"
    );
}
