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
    round_trip::<AccountId>(r#""alice""#);
    round_trip::<ContentHash>(
        r#""sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855""#,
    );
}

#[test]
fn session_id_must_be_lowercase_uuid_text() {
    rejected::<SessionId>(r#""0192F3A0-1111-7abc-8def-001122334455""#, "小写十六进制");
    rejected::<SessionId>(r#""0192f3a0-1111-7abc-8def-00112233445""#, "36 个字符");
    rejected::<SessionId>(r#""0192f3a0_1111-7abc-8def-001122334455""#, "要是 -");
}

#[test]
fn command_id_is_short_printable_text() {
    rejected::<CommandId>(r#""""#, "不能是空的");
    rejected::<CommandId>(&format!("\"{}\"", "x".repeat(129)), "128 字节");
    rejected::<CommandId>(r#""cmd\n1""#, "控制字符");
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

#[test]
fn account_is_like_a_linux_login_name() {
    let longest = "a".repeat(32);
    for good in ["a", "alice", "bob_2-x", longest.as_str()] {
        assert!(AccountId::parse(good).is_ok(), "{good} 应该读得进来");
    }
    rejected::<AccountId>(r#""Alice""#, "小写英文字母开头");
    rejected::<AccountId>(r#""1abc""#, "小写英文字母开头");
    rejected::<AccountId>(r#""小明""#, "小写英文字母开头");
    rejected::<AccountId>(r#""a b""#, "只能用小写字母");
    rejected::<AccountId>(r#""aB""#, "只能用小写字母");
    rejected::<AccountId>(&format!("\"{}\"", "a".repeat(33)), "最长 32");
    rejected::<AccountId>(r#""con""#, "Windows 保留");
    rejected::<AccountId>(r#""lpt9""#, "Windows 保留");
}

#[test]
fn content_hash_is_sha256_in_lowercase_hex() {
    let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    rejected::<ContentHash>(&format!("\"SHA256:{hex}\""), "sha256: 开头");
    rejected::<ContentHash>(
        &format!("\"sha256:{}\"", hex.to_uppercase()),
        "小写十六进制",
    );
    rejected::<ContentHash>(&format!("\"sha256:{}\"", &hex[1..]), "64 位");
}

#[test]
fn seq_starts_at_one() {
    rejected::<Seq>("0", "从 1 开始");
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
    rejected::<TurnId>("0", "从 1 开始");
}

#[test]
fn error_says_what_why_and_what_was_read() {
    let err = SessionId::parse("x").unwrap_err();
    assert_eq!(
        err.to_string(),
        "会话编号的写法不对：要 36 个字符（读到的是 \"x\"）"
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
}

#[test]
fn module_and_driver_names_follow_the_account_rule() {
    rejected::<ModuleId>(r#""Memory""#, "小写英文字母开头");
    rejected::<DriverFamily>(r#""openai.chat""#, "只能用小写字母");
    rejected::<ModuleId>(r#""nul""#, "Windows 保留");
}

#[test]
fn short_names_are_opaque_but_bounded() {
    round_trip::<ModelName>(r#""qwen/qwen3-235b-a22b@2026-07""#);
    rejected::<ProviderId>(r#""""#, "不能是空的");
    rejected::<VenueId>(&format!("\"{}\"", "v".repeat(129)), "128 字节");
    rejected::<ExternalId>(r#""qq:\u000710086""#, "控制字符");
}

#[test]
fn media_type_is_lowercase_type_slash_subtype() {
    round_trip::<MediaType>(
        r#""application/vnd.openxmlformats-officedocument.wordprocessingml.document""#,
    );
    rejected::<MediaType>(r#""image""#, "类型/子类型");
    rejected::<MediaType>(r#""Image/PNG""#, "小写字母");
    rejected::<MediaType>(r#""image/png/x""#, "小写字母");
    rejected::<MediaType>(r#""image/""#, "小写字母");
}

#[test]
fn file_name_is_a_name_not_a_path() {
    rejected::<FileName>(r#""../etc/passwd""#, "/");
    rejected::<FileName>(r#""a\\b.txt""#, "/");
    rejected::<FileName>(r#""..""#, ". 或 ..");
    rejected::<FileName>(r#""""#, "不能是空的");
    rejected::<FileName>(&format!("\"{}\"", "报".repeat(86)), "255 字节");
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
    rejected::<EventKind>(r#""""#, "不能是空的");
    rejected::<EventKind>(r#""message""#, "至少两段");
    rejected::<EventKind>(r#""Message.user""#, "小写字母开头");
    rejected::<EventKind>(r#"".user""#, "小写字母开头");
    rejected::<EventKind>(r#""message..user""#, "小写字母开头");
    rejected::<EventKind>(r#""message.9user""#, "小写字母开头");
    rejected::<EventKind>(r#""message.us er""#, "只能用小写字母");
    rejected::<EventKind>(&format!("\"a.{}\"", "b".repeat(127)), "128 字节");
}
