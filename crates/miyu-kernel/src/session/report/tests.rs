//! 截正文（施工 7-6，`docs/blueprint/agents.md` 第二条第 3 条）：到上限的原样，过了的留头尾各一半，中间一行写省了多少个字。
//! 数的是字，不是字节。

use super::*;
use crate::template::Template;

fn reports(chars: usize) -> Reports {
    Reports {
        chars,
        omitted: Template::parse("[... {count} characters omitted ...]\n").unwrap(),
    }
}

#[test]
fn a_text_up_to_the_limit_is_kept_whole() {
    assert_eq!(reports(4).cut(""), (String::new(), false));
    assert_eq!(reports(4).cut("一二三四"), ("一二三四".to_string(), false));
}

#[test]
fn a_longer_text_keeps_half_its_limit_from_each_end() {
    assert_eq!(
        reports(4).cut("一二三四五六七"),
        (
            "一二\n[... 3 characters omitted ...]\n六七".to_string(),
            true
        )
    );
    // 上限是单数的，尾巴多一个字。
    assert_eq!(
        reports(3).cut("abcdefg"),
        ("a\n[... 4 characters omitted ...]\nfg".to_string(), true)
    );
}

#[test]
fn an_empty_omitted_line_leaves_a_line_break() {
    let bare = Reports {
        chars: 2,
        omitted: Template::parse("").unwrap(),
    };
    assert_eq!(bare.cut("abcd"), ("a\nd".to_string(), true));
}
