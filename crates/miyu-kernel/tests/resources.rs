//! 出厂的英文资源（`resources/core/`）读得进来：内核在执行之前就拦下时写给模型的那两句
//! （`docs/designs/02-内核.md` 第六节「工具怎么调、下一步怎么走」）。资源在编译时拿进来，
//! 纯逻辑门禁只扫 `src/`，集成测试可以。

use miyu_kernel::tool::ToolTexts;

#[test]
fn the_tool_result_sentences_are_usable() {
    let texts = ToolTexts::new(
        include_str!("../../../resources/core/tool-results/unknown.txt"),
        include_str!("../../../resources/core/tool-results/not-an-object.txt"),
    )
    .expect("出厂的两句用得了");
    assert_eq!(texts.unknown("reed"), "There is no tool named \"reed\".\n");
    assert_eq!(
        texts.not_an_object("read"),
        "The arguments for \"read\" are not a JSON object.\n"
    );
}
