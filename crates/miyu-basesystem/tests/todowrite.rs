//! `todowrite`（`docs/blueprint/tools/todowrite.md`，施工 D-3）：整份换，结果一句短话、逐字节比，效果带着整份；全部做完的
//! 清空，效果是空列表；参数不对的（少了格、状态不认识）报参数不对、什么效果都不报；访问类别读。给人看的说法中文、英文都有。

mod support;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Todo, TodoStatus, TodoWritten};
use miyu_kernel::tool::Access;
use miyu_tool::{Done, Effect, Stop};

use support::{Site, check, human, readable, said, tool};

fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

async fn write(args: serde_json::Value) -> Done {
    Site::new()
        .done_with_usage("todowrite", args, None, Stop::default())
        .await
}

fn todo(content: &str, status: TodoStatus) -> Todo {
    Todo {
        content: content.to_string(),
        status,
    }
}

#[tokio::test]
async fn the_whole_list_is_replaced_and_reported_in_one_line() {
    let done = write(json!({"todos": [
        {"content": "读代码", "status": "completed"},
        {"content": "写测试", "status": "in_progress"},
        {"content": "跑门禁", "status": "pending"}
    ]}))
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Todo list updated: 1 of 3 done.\n");
    assert_eq!(
        done.effects,
        [Effect::TodoWritten(TodoWritten {
            todos: vec![
                todo("读代码", TodoStatus::Completed),
                todo("写测试", TodoStatus::InProgress),
                todo("跑门禁", TodoStatus::Pending),
            ]
        })]
    );
    let mut checked = Vec::new();
    check(
        &mut checked,
        human(done),
        said("todowrite/updated")
            .with("done", "1")
            .with("total", "3"),
    );
    readable(&checked, &["todowrite"]);
}

#[tokio::test]
async fn all_done_or_empty_clears_the_list() {
    for args in [
        json!({"todos": [{"content": "a", "status": "completed"}, {"content": "b", "status": "completed"}]}),
        json!({"todos": []}),
    ] {
        let done = write(args.clone()).await;
        assert!(!done.error, "{args}");
        assert_eq!(
            text(&done),
            "All todos are done. The list is cleared.\n",
            "{args}"
        );
        assert_eq!(
            done.effects,
            [Effect::TodoWritten(TodoWritten { todos: Vec::new() })],
            "{args}"
        );
        let mut checked = Vec::new();
        check(&mut checked, human(done), said("todowrite/cleared"));
        readable(&checked, &[]);
    }
}

#[tokio::test]
async fn bad_arguments_report_no_effect() {
    for args in [
        json!({}),
        json!({"todos": [{"content": "a"}]}),
        json!({"todos": [{"content": "a", "status": "doing"}]}),
        json!({"todos": [{"status": "pending"}]}),
    ] {
        let done = write(args.clone()).await;
        assert!(done.error, "{args}");
        assert!(
            text(&done).starts_with("The arguments are not right"),
            "{args}"
        );
        assert!(done.effects.is_empty(), "{args}");
    }
}

#[test]
fn it_only_reads() {
    assert_eq!(tool("todowrite").spec().access, Access::Read);
}
