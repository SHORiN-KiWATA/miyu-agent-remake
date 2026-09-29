//! 代码着色（蓝图 `tui.md`「代码着色」）：每一样认得对、跨行的接得上、一个字都不丢。

use ratatui::style::Style;

use super::{Highlighter, Languages};
use crate::theme;

fn languages() -> Languages {
    serde_json::from_str(include_str!("../../../resources/code.json")).unwrap()
}

/// 一段代码照 `lang` 逐行着色，交回每一段的字和样子；顺手查一个字都不丢。
fn colored(code: &str, lang: &str) -> Vec<(String, Style)> {
    let langs = languages();
    let mut h = Highlighter::new(langs.find(lang));
    let mut out = Vec::new();
    for line in code.split('\n') {
        let pieces = h.line(line);
        let joined: String = pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(joined, line, "一个字都不丢");
        out.extend(pieces.into_iter().map(|p| (p.text, p.style)));
    }
    out
}

fn style(pieces: &[(String, Style)], text: &str) -> Option<Style> {
    pieces.iter().find(|(t, _)| t == text).map(|(_, s)| *s)
}

#[test]
fn keywords_functions_strings_numbers_comments() {
    let p = colored(r#"def norm(x): return "a\"b" + 0.5  # done"#, "Py");
    assert_eq!(style(&p, "def"), Some(theme::code_keyword()));
    assert_eq!(style(&p, "norm"), Some(theme::code_function()));
    assert_eq!(style(&p, r#""a\"b""#), Some(theme::code_string()));
    assert_eq!(style(&p, "0.5"), Some(theme::code_number()));
    assert_eq!(style(&p, "# done"), Some(theme::code_comment()));
}

#[test]
fn unknown_languages_still_get_strings_and_numbers() {
    let p = colored(r#"x = "s" 42"#, "没登记的");
    assert_eq!(style(&p, r#""s""#), Some(theme::code_string()));
    assert_eq!(style(&p, "42"), Some(theme::code_number()));
}

#[test]
fn rust_types_macros_attributes_constants_operators() {
    let p = colored(
        "#[derive(Debug)]\nlet v: Vec<String> = vec![MAX_SIZE, 1]; // 注释\nfn f() -> i32 { true }",
        "rs",
    );
    assert_eq!(
        style(&p, "#[derive(Debug)]"),
        Some(theme::code_macro()),
        "属性"
    );
    assert_eq!(style(&p, "let"), Some(theme::code_keyword()));
    assert_eq!(style(&p, "Vec"), Some(theme::code_type()), "大写开头的");
    assert_eq!(style(&p, "String"), Some(theme::code_type()));
    assert_eq!(style(&p, "vec!"), Some(theme::code_macro()), "宏");
    assert_eq!(
        style(&p, "MAX_SIZE"),
        Some(theme::code_constant()),
        "全大写"
    );
    assert_eq!(style(&p, "="), Some(theme::code_operator()));
    assert_eq!(style(&p, "// 注释"), Some(theme::code_comment()));
    assert_eq!(style(&p, "i32"), Some(theme::code_type()), "内建类型");
    assert_eq!(style(&p, "->"), Some(theme::code_operator()));
    assert_eq!(style(&p, "true"), Some(theme::code_constant()));
}

#[test]
fn block_comments_run_across_lines() {
    let p = colored("/* 一\n二 */ let x", "rust");
    assert_eq!(style(&p, "/* 一"), Some(theme::code_comment()));
    assert_eq!(
        style(&p, "二 */"),
        Some(theme::code_comment()),
        "接到下一行"
    );
    assert_eq!(
        style(&p, "let"),
        Some(theme::code_keyword()),
        "注释完了照常认"
    );
}

#[test]
fn python_decorators_and_triple_quoted_strings() {
    let p = colored("@dataclass\nx = \"\"\"doc\nmore\"\"\" + None", "python");
    assert_eq!(style(&p, "@dataclass"), Some(theme::code_macro()));
    assert_eq!(style(&p, "\"\"\"doc"), Some(theme::code_string()));
    assert_eq!(
        style(&p, "more\"\"\""),
        Some(theme::code_string()),
        "三引号跨行"
    );
    assert_eq!(style(&p, "None"), Some(theme::code_constant()));
}

#[test]
fn keys_are_told_apart_from_values() {
    let j = colored(r#"{"name": "x", "n": 1, "ok": true}"#, "json");
    assert_eq!(style(&j, r#""name""#), Some(theme::code_property()));
    assert_eq!(style(&j, r#""x""#), Some(theme::code_string()));
    assert_eq!(style(&j, r#""n""#), Some(theme::code_property()));
    assert_eq!(style(&j, "true"), Some(theme::code_constant()));
    let y = colored("key: value\n- item: 2", "yaml");
    assert_eq!(style(&y, "key"), Some(theme::code_property()));
    assert_eq!(style(&y, "item"), Some(theme::code_property()));
    let t = colored("[server]\nport = 80", "toml");
    assert_eq!(style(&t, "[server]"), Some(theme::code_type()), "段");
    assert_eq!(style(&t, "port"), Some(theme::code_property()));
    assert_eq!(style(&t, "80"), Some(theme::code_number()));
    let c = colored("a { color: red; }", "css");
    assert_eq!(style(&c, "color"), Some(theme::code_property()));
}

#[test]
fn shell_variables_and_flags() {
    let p = colored("rm $HOME/tmp ${X} --force -v", "bash");
    assert_eq!(style(&p, "$HOME"), Some(theme::code_macro()));
    assert_eq!(style(&p, "${X}"), Some(theme::code_macro()));
    assert_eq!(style(&p, "--force"), Some(theme::code_parameter()));
    assert_eq!(style(&p, "-v"), Some(theme::code_parameter()));
    // 双引号里的变量会展开：单独着色；单引号里的不展开，照字符串。
    let q = colored(r#"echo "hi $USER, ${HOME}!" 'no $X'"#, "bash");
    assert_eq!(style(&q, "$USER"), Some(theme::code_macro()));
    assert_eq!(style(&q, "${HOME}"), Some(theme::code_macro()));
    assert_eq!(style(&q, r#""hi "#), Some(theme::code_string()));
    assert_eq!(style(&q, "'no $X'"), Some(theme::code_string()));
}

#[test]
fn diff_lines_are_added_and_removed() {
    let p = colored("--- a/x\n+++ b/x\n@@ -1 +1 @@\n-旧的\n+新的\n 不变", "diff");
    assert_eq!(style(&p, "-旧的"), Some(theme::removed()));
    assert_eq!(style(&p, "+新的"), Some(theme::added()));
    assert_eq!(style(&p, "@@ -1 +1 @@"), Some(theme::code_macro()));
    assert_eq!(style(&p, "--- a/x"), Some(theme::code_macro()), "文件头");
    assert_eq!(style(&p, " 不变"), Some(Style::new()));
}

#[test]
fn html_tags_and_attributes() {
    let p = colored(r#"<div class="a">x</div> <!-- 注 -->"#, "html");
    assert_eq!(style(&p, "div"), Some(theme::code_keyword()));
    assert_eq!(style(&p, "class"), Some(theme::code_property()));
    assert_eq!(style(&p, r#""a""#), Some(theme::code_string()));
    assert_eq!(style(&p, "<!-- 注 -->"), Some(theme::code_comment()));
}

#[test]
fn typescript_is_known() {
    let p = colored("interface Foo { bar: string }", "ts");
    assert_eq!(style(&p, "interface"), Some(theme::code_keyword()));
    assert_eq!(style(&p, "Foo"), Some(theme::code_type()));
    assert_eq!(style(&p, "string"), Some(theme::code_type()));
}

#[test]
fn c_preprocessor_directives() {
    let p = colored("#include <stdio.h>\n  #define MAX 10", "c");
    assert_eq!(style(&p, "#include"), Some(theme::code_macro()));
    assert_eq!(
        style(&p, "#define"),
        Some(theme::code_macro()),
        "前面有空白也认"
    );
    assert_eq!(style(&p, "MAX"), Some(theme::code_constant()));
}
