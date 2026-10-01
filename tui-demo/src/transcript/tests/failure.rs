//! 出错那一句（蓝图 `tui.md`「正文」第 4 条）：供应商的原话照样写，几种状态码前面加人话，内核自己查出来的写分类。

use super::apply;
use crate::core::{CallError, EndReason, Push};
use crate::transcript::{Kind, Transcript};

/// 一轮以这个错结束，交回正文最后一条。
fn failed(class: &str, message: &str) -> (Kind, String) {
    ended(vec![Push::CallFailed(CallError {
        class: class.into(),
        message: message.into(),
        status: None,
    })])
}

/// 一轮开始、收到这几条、以出错结束，交回正文最后一条。
fn ended(pushes: Vec<Push>) -> (Kind, String) {
    let mut t = Transcript::default();
    let mut all = vec![Push::TurnStarted(1, None)];
    all.extend(pushes);
    all.push(Push::TurnEnded(EndReason::Error));
    apply(&mut t, all);
    let last = t.entries.last().unwrap();
    (last.kind.clone(), last.text.clone())
}

#[test]
fn the_providers_words_go_out_as_they_are() {
    // 2026-09-30 项目主人：各家的错误码不一样，原话大多已经说清了为什么，不加分类。
    assert_eq!(
        failed(
            "auth",
            "HTTP 401: Authentication Fails, Your api key: ****0000 is invalid "
        ),
        (
            Kind::Error,
            "出错了：HTTP 401: Authentication Fails, Your api key: ****0000 is invalid".into()
        )
    );
    assert_eq!(
        failed("other", "HTTP 400: Model Not Exist").1,
        "出错了：HTTP 400: Model Not Exist"
    );
}

#[test]
fn rate_limits_get_a_plain_word_in_front() {
    assert_eq!(
        failed("rate_limited", "HTTP 429: Rate limit reached").1,
        "出错了：被限速了，或者额度不够，过一会儿再试：HTTP 429: Rate limit reached"
    );
}

#[test]
fn the_kernels_own_errors_say_only_their_class_in_plain_words() {
    // 2026-10-01 项目主人：已经有中文的报错了，后面不要再接英文。内核自己查出来的原话是给运行日志的诊断，不接。
    assert_eq!(failed("empty_reply", "").1, "出错了：回复是空的");
    assert_eq!(
        failed("bad_stream", "delta for unknown block 3").1,
        "出错了：回复的流不对"
    );
    // 核心 8-6：没配好模型（`no_model`）。不写怎么配（2026-10-01 项目主人去掉的）。
    assert_eq!(
        failed("no_model", "no model configured: set models.chat").1,
        "出错了：没有可用的模型"
    );
    // 供应商的照旧：原话是空的写分类。
    assert_eq!(
        failed("auth", "  ").1,
        "出错了：认证失败",
        "原话是空的写分类"
    );
}

#[test]
fn a_status_the_core_gives_picks_the_plain_word() {
    let with = |class: &str, status: u16, message: &str| {
        ended(vec![Push::CallFailed(CallError {
            class: class.into(),
            message: message.into(),
            status: Some(status),
        })])
        .1
    };
    assert_eq!(
        with("other", 404, "HTTP 404: Not Found"),
        "出错了：找不到，检查端点地址和模型名：HTTP 404: Not Found"
    );
    assert_eq!(
        with("auth", 402, "HTTP 402: Insufficient Balance"),
        "出错了：额度用完了：HTTP 402: Insufficient Balance"
    );
    assert_eq!(
        with("auth", 401, "HTTP 401: invalid key"),
        "出错了：HTTP 401: invalid key",
        "别的状态码照写原话"
    );
}

#[test]
fn a_call_that_went_through_later_clears_the_earlier_error() {
    // 2026-09-30 查出：前面报过错、后来成了，这一轮因为别的出错结束时写成了前面那个。
    let (_, text) = ended(vec![
        Push::CallFailed(CallError {
            class: "context_too_long".into(),
            message: "HTTP 400: maximum context length".into(),
            status: Some(400),
        }),
        Push::CallOk,
    ]);
    assert_eq!(text, "出错了：模型出错", "不写前面那个旧错");
}
