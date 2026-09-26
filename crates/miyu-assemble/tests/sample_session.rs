//! 样本会话组装出样本请求（`docs/designs/08-上下文投影.md` 第四节「默认的组装怎么写」）：
//! 事件的样本（`docs/designs/samples/events/`）讲的是同一个会话，喂进有效历史，
//! 组装出来的请求逐字节等于请求的样本（`docs/designs/samples/requests/`）。
//!
//! - 喂到 47 号为止，是 42 号回合里工具结果回来以后的那一次请求（`second-step.json`）。46 号是
//!   第一次请求的 `model.called`，不渲染；
//! - 喂到 54 号为止，是压缩以后的样子，只剩检查点（`after-compaction.json`）；
//! - 全部喂进去：压缩以后那一轮里排着队、又被撤回的那句话，不在请求里。
//!
//! 出厂的英文用资源目录里的真文件，在编译时拿进来。样本是图纸的一部分，住在设计文档旁边，
//! 所以要读文件；纯逻辑门禁只扫 `src/`，集成测试可以读。样本的序号中间有空当（省掉了
//! 前面的几十条），过不了账本，所以直接交给有效历史。

use std::fs;
use std::path::PathBuf;

use miyu_assemble::{DefaultAssembler, Stable, Texts, TurnEndedTexts};
use miyu_kernel::assemble::Assembler;
use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::ToolSpec;

/// 样本会话的工具面：只有一件 `read`。
const READ: &str = "Read a text file by line pages, an image, a PDF, or list a directory. Prefer this over `cat` in the shell: files read here come back after compaction.";
const READ_PARAMETERS: &str = r#"{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}"#;

/// 出厂的英文，资源目录里的真文件。
fn texts() -> Texts {
    Texts {
        checkpoint_open: include_str!("../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../resources/core/checkpoint-close.txt").to_string(),
        turn_ended: TurnEndedTexts {
            interrupted: include_str!("../../../resources/core/turn-ended/interrupted.txt")
                .to_string(),
            error: include_str!("../../../resources/core/turn-ended/error.txt").to_string(),
            step_limit: include_str!("../../../resources/core/turn-ended/step_limit.txt")
                .to_string(),
            aborted: include_str!("../../../resources/core/turn-ended/aborted.txt").to_string(),
        },
    }
}

/// 样本会话的组装器：一件 `read` 工具，一句 system，没有示范对话。
fn assembler() -> DefaultAssembler {
    let parameters: RawJson = serde_json::from_str(READ_PARAMETERS).expect("参数格式是 JSON");
    let stable = Stable {
        tools: vec![ToolSpec {
            name: "read".to_string(),
            description: READ.to_string(),
            parameters,
        }],
        system: "You are a helpful software engineer.".to_string(),
        demos: vec![],
    };
    DefaultAssembler::new(stable, texts())
}

/// 仓库根：这个 crate 的目录往上两级。
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 样本会话的全部事件，照序号排好。
fn events() -> Vec<Event> {
    let dir = root().join("docs/designs/samples/events");
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|e| panic!("读不了样本目录 {}：{e}", dir.display()));
    let mut events = Vec::new();
    for entry in entries {
        let path = entry.expect("列样本目录时出错").path();
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        for line in text.lines() {
            let event = Event::from_line(line)
                .unwrap_or_else(|e| panic!("{} 读不出来：{e}", path.display()));
            events.push(event);
        }
    }
    events.sort_by_key(|event| event.seq);
    events
}

/// 请求的样本，去掉行尾的换行。
fn sample(name: &str) -> String {
    let path = root().join("docs/designs/samples/requests").join(name);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    text.strip_suffix('\n')
        .unwrap_or_else(|| panic!("{name} 要以一个换行结尾"))
        .to_string()
}

/// 把序号不超过 `upto` 的样本事件依次交给有效历史，组装出请求的规范字节。
fn assembled_upto(upto: u64) -> String {
    let mut history = History::default();
    for event in events() {
        if event.seq.get() <= upto {
            history.append(event);
        }
    }
    let bytes = assembler().assemble(&history).canonical_bytes();
    String::from_utf8(bytes).expect("规范的字节是 UTF-8")
}

#[test]
fn the_session_up_to_the_tool_result_is_the_second_step() {
    assert_eq!(assembled_upto(47), sample("second-step.json"));
}

#[test]
fn the_session_up_to_the_compaction_is_the_checkpoint_alone() {
    assert_eq!(assembled_upto(54), sample("after-compaction.json"));
}

#[test]
fn the_withdrawn_message_never_reaches_the_request() {
    let whole = assembled_upto(u64::MAX);
    assert!(whole.contains("再看看 tests 目录"), "{whole}");
    assert!(!whole.contains("README"), "撤回的那句不该在请求里：{whole}");
}

/// 请人确认和人的决定不进上下文，她看到的只有工具结果：被拒绝的那一句、带着理由在请求里，
/// 请求里给头看的说明不在。
#[test]
fn only_the_denied_result_of_an_approval_reaches_the_request() {
    let whole = assembled_upto(u64::MAX);
    assert!(
        whole.contains("the user denied it and said"),
        "被拒绝的结果要在请求里：{whole}"
    );
    assert!(
        !whole.contains("outside_workspace"),
        "请求给头看的说明不该在请求里：{whole}"
    );
}
