//! 参考文件的样子：开头两行，表照字母先后、表里的项也照字母先后，每一项两行注释一行默认值，空一行隔开；缺字报缺
//! 了哪一句。和出厂的清单、资源生成的样本逐字节比，在 `miyu-core/tests/settings.rs`：这里拿不到上层的清单。

use crate::reference::render;
use crate::test_support::{item, system_only, words};
use crate::words::Missing;

const EXPECTED: &str = "# 头一行。
# 头两行。

[a]
# a.a 的名字：a.a 的说明。
# 能写：p 或 q。只能写在系统配置或个人设置里。立即生效。
a = \"p\"

# a.d 的名字：a.d 的说明。
# 能写：on 或 off。只能写在系统配置里。立即生效。
d = \"on\"

[a.b]
# a.b.c 的名字：a.b.c 的说明。
# 能写：x\" 或 \"y\"。只能写在系统配置或个人设置里。立即生效。
c = \"x\\\"\"

[z]
# z.a 的名字：z.a 的说明。
# 能写：m 或 n。只能写在系统配置或个人设置里。立即生效。
a = \"m\"
";

#[test]
fn tables_and_items_go_in_order_with_two_comment_lines_each() {
    // 登记的先后打乱：表照名字排，`a` 在 `a.b` 前面，`a.d` 不接在 `a.b.c` 后面另起一张 `[a]`。
    let items = [
        item("z.a", &["m", "n"], "m"),
        item("a.b.c", &["x\"", "\"y\""], "x\""),
        system_only(item("a.d", &["on", "off"], "on")),
        item("a.a", &["p", "q"], "p"),
    ];
    assert_eq!(render(&items, &words(&items)).as_deref(), Ok(EXPECTED));
}

#[test]
fn nothing_registered_leaves_only_the_head() {
    assert_eq!(
        render(&[], &words(&[])).as_deref(),
        Ok("# 头一行。\n# 头两行。\n")
    );
}

#[test]
fn every_line_of_a_sentence_is_a_comment() {
    let items = [item("a.a", &["p", "q"], "p")];
    let mut words = words(&items);
    words
        .sentences
        .insert("config/reference-header".to_string(), "一\n二".to_string());
    let text = render(&items, &words).unwrap_or_default();
    assert!(text.starts_with("# 一\n# 二\n# 头两行。\n"), "{text}");
}

#[test]
fn missing_words_are_named() {
    let items = [item("a.a", &["p", "q"], "p")];
    for key in [
        "config/reference-header",
        "config/reference-where",
        "config/reference-item",
        "config/facts",
    ] {
        let mut words = words(&items);
        words.sentences.remove(key);
        assert_eq!(
            render(&items, &words),
            Err(Missing(key.to_string())),
            "{key}"
        );
    }
}
