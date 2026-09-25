//! 纯逻辑门禁：纯逻辑那几层的 `src/` 里不许有 I/O，也不许用遍历顺序会变的容器
//! （01-架构 第九节，规则 3）。
//!
//! 按文本找，注释不算。它防的是手滑，不是存心绕过。

/// 找什么，以及找到了怎么说。
const BANNED: &[(&str, &str)] = &[
    ("std::fs", "读写文件"),
    ("std::net", "联网"),
    ("std::process", "起子进程"),
    ("std::env", "读环境变量"),
    ("std::thread", "开线程"),
    ("std::os", "调平台的系统接口"),
    ("env!", "编译时读环境变量"),
    ("option_env!", "编译时读环境变量"),
    ("SystemTime", "读时钟"),
    ("Instant", "读时钟"),
    ("stdin()", "读标准输入"),
    ("stdout()", "写标准输出"),
    ("stderr()", "写标准错误"),
    ("print!", "写标准输出"),
    ("println!", "写标准输出"),
    ("eprint!", "写标准错误"),
    ("eprintln!", "写标准错误"),
    ("dbg!", "写标准错误"),
    ("HashMap", "遍历顺序每次运行都不一样，改用 BTreeMap"),
    ("HashSet", "遍历顺序每次运行都不一样，改用 BTreeSet"),
];

/// `use std::{fs, io};` 这种合在一起的写法里，要拦的那几个名字。
const BANNED_IN_STD_GROUP: &[&str] = &["fs", "net", "process", "env", "thread", "os"];

/// 扫一个文件。`label` 用在报错里，通常是相对仓库根的路径。
pub fn scan(label: &str, source: &str) -> Vec<String> {
    let code = without_comments(source);
    let mut found = Vec::new();
    for (token, why) in BANNED {
        for at in find_token(&code, token) {
            let line = line_of(&code, at);
            found.push((line, format!("{label}:{line}：`{token}`，{why}（规则 3）")));
        }
    }
    for (at, name) in std_group_names(&code) {
        if BANNED_IN_STD_GROUP.contains(&name.as_str()) {
            let line = line_of(&code, at);
            found.push((
                line,
                format!("{label}:{line}：`use std::{{..{name}..}}`，纯逻辑的层不许用 std::{name}（规则 3）"),
            ));
        }
    }
    found.sort();
    found.into_iter().map(|(_, problem)| problem).collect()
}

/// 去掉每一行 `//` 之后的部分，行数不变。
fn without_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 整词出现的位置：前面不能紧挨着名字里的字符；以字母结尾的，后面也不能。
fn find_token(code: &str, token: &str) -> Vec<usize> {
    let ends_like_name = token.ends_with(is_ident);
    code.match_indices(token)
        .filter(|(at, _)| {
            let before = code[..*at].chars().next_back();
            let after = code[at + token.len()..].chars().next();
            !before.is_some_and(is_ident) && !(ends_like_name && after.is_some_and(is_ident))
        })
        .map(|(at, _)| at)
        .collect()
}

/// `std::{...}` 花括号里出现的每一个名字，连同花括号的位置。
fn std_group_names(code: &str) -> Vec<(usize, String)> {
    let mut names = Vec::new();
    for at in find_token(code, "std::{") {
        let open = at + "std::".len();
        let mut depth = 0;
        let mut end = code.len();
        for (i, c) in code[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let inside = &code[open + 1..end];
        for name in inside
            .split(|c: char| !is_ident(c))
            .filter(|n| !n.is_empty())
        {
            names.push((at, name.to_string()));
        }
    }
    names
}

fn line_of(code: &str, at: usize) -> usize {
    code[..at].matches('\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_source_passes() {
        let source = "use std::collections::BTreeMap;\npub fn f(m: &BTreeMap<u8, u8>) -> usize { m.len() }\n";
        assert!(scan("lib.rs", source).is_empty());
    }

    #[test]
    fn std_fs_is_rejected_with_line_number() {
        let problems = scan("lib.rs", "//! 说明\n\nuse std::fs;\n");
        assert_eq!(problems, vec!["lib.rs:3：`std::fs`，读写文件（规则 3）"]);
    }

    #[test]
    fn grouped_std_import_is_rejected() {
        let problems = scan("lib.rs", "use std::{\n    io::Write,\n    fs,\n};\n");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("std::fs"));
    }

    #[test]
    fn println_is_rejected() {
        let problems = scan("lib.rs", "pub fn hello() { println!(\"hi\"); }\n");
        assert_eq!(problems, vec!["lib.rs:1：`println!`，写标准输出（规则 3）"]);
    }

    #[test]
    fn eprintln_is_reported_once() {
        let problems = scan("lib.rs", "fn f() { eprintln!(\"x\"); }\n");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("eprintln!"));
    }

    #[test]
    fn clock_is_rejected() {
        let problems = scan(
            "lib.rs",
            "use std::time::Instant;\nfn f() { let _t = Instant::now(); }\n",
        );
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn hash_map_is_rejected() {
        let problems = scan("lib.rs", "use std::collections::HashMap;\n");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("BTreeMap"));
    }

    #[test]
    fn comments_are_ignored() {
        let source =
            "// 不用 std::fs，也不 println!\n/// 读时钟（Instant）是执行器的事\nfn f() {}\n";
        assert!(scan("lib.rs", source).is_empty());
    }

    #[test]
    fn similar_names_are_not_rejected() {
        let source = "fn instantiate() {}\nstruct Instantiate;\nfn f(o: &Out) { let _ = o.stdout_len; }\nuse std::ops::Add;\n";
        assert!(
            scan("lib.rs", source).is_empty(),
            "{:?}",
            scan("lib.rs", source)
        );
    }
}
