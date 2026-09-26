//! 出厂的英文资源（`resources/core/`）读得进来：内核替工具写给模型的几句（`docs/designs/02-内核.md`
//! 第六节「工具怎么调、下一步怎么走」「打断和急着插话」）。资源在编译时拿进来，纯逻辑门禁
//! 只扫 `src/`，集成测试可以。

use miyu_kernel::tool::{ToolTextSources, ToolTexts};

#[test]
fn the_tool_result_sentences_are_usable() {
    let texts = ToolTexts::new(ToolTextSources {
        unknown: include_str!("../../../resources/core/tool-results/unknown.txt"),
        not_an_object: include_str!("../../../resources/core/tool-results/not-an-object.txt"),
        cancelled_before: include_str!("../../../resources/core/tool-results/cancelled-before.txt"),
        cancelled_running: include_str!(
            "../../../resources/core/tool-results/cancelled-running.txt"
        ),
        skipped: include_str!("../../../resources/core/tool-results/skipped.txt"),
    })
    .expect("出厂的几句用得了");
    assert_eq!(texts.unknown("reed"), "There is no tool named \"reed\".\n");
    assert_eq!(
        texts.not_an_object("read"),
        "The arguments for \"read\" are not a JSON object.\n"
    );
    assert!(texts.cancelled_running().contains("partly done"));
    assert!(texts.skipped().starts_with("The call was skipped"));
}
