//! 参数写错时说的那一句（施工 4-11，`docs/blueprint/cli/main.md`「参数写错时」）：照 clap 报的错分几种，每种
//! 一句，跟着界面语言，不带 clap 的用法和提示。主程序把它印在标准错误上，退出码 2。

use clap::error::{ContextKind, ContextValue, ErrorKind};

use miyu_store::human::clean;

use crate::language::Language;

/// clap 报的错说成一句话。帮助、版本不是错，主程序自己印，不走这里。
pub fn misuse(error: &clap::Error, language: Language) -> String {
    let chinese = language == Language::Chinese;
    let arg = clean(&first(error, ContextKind::InvalidArg));
    match error.kind() {
        ErrorKind::InvalidSubcommand => {
            language.no_such_command(&clean(&first(error, ContextKind::InvalidSubcommand)))
        }
        // 必写的只有 `ask` 的要说的话：测试查着，多了一个，这里要跟着改。
        ErrorKind::MissingRequiredArgument => match chinese {
            true => "少了要说的话：miyu ask \"…\"".to_string(),
            false => "Missing what to say: miyu ask \"…\"".to_string(),
        },
        ErrorKind::UnknownArgument => match (chinese, arg.starts_with('-')) {
            (true, true) => format!("没有 {arg} 这个选项"),
            (true, false) => format!("多了参数：{arg}"),
            (false, true) => format!("No such option: {arg}"),
            (false, false) => format!("Unexpected argument: {arg}"),
        },
        ErrorKind::ArgumentConflict => {
            let prior = clean(&first(error, ContextKind::PriorArg));
            let (one, other) = (option(&arg), option(&prior));
            match chinese {
                true => format!("{one} 和 {other} 只能写一个"),
                false => format!("{one} and {other} can't be used together"),
            }
        }
        ErrorKind::InvalidValue if first(error, ContextKind::InvalidValue).is_empty() => {
            match chinese {
                true => format!("{} 后面少了值", option(&arg)),
                false => format!("{} needs a value", option(&arg)),
            }
        }
        ErrorKind::InvalidValue if !all(error, ContextKind::ValidValue).is_empty() => {
            let valid = all(error, ContextKind::ValidValue);
            match chinese {
                true => format!("{} 只能是 {}", option(&arg), listed(&valid, "、", " 或 ")),
                false => format!("{} must be {}", option(&arg), listed(&valid, ", ", " or ")),
            }
        }
        _ => {
            let said = clean(clap_said(error).trim_start_matches("error: "));
            match chinese {
                true => format!("参数不对：{said}"),
                false => format!("Bad arguments: {said}"),
            }
        }
    }
}

/// 报错里这一样的第一个字：一个的就是它，几个的取第一个，没有的是空的。
fn first(error: &clap::Error, kind: ContextKind) -> String {
    all(error, kind).into_iter().next().unwrap_or_default()
}

/// 报错里这一样的全部字。
fn all(error: &clap::Error, kind: ContextKind) -> Vec<String> {
    match error.get(kind) {
        Some(ContextValue::String(one)) => vec![one.clone()],
        Some(ContextValue::Strings(many)) => many.clone(),
        _ => Vec::new(),
    }
}

/// 选项去掉后面的值名：`--session <SESSION>` 写成 `--session`。
fn option(arg: &str) -> &str {
    arg.split_whitespace().next().unwrap_or(arg)
}

/// 几个值连起来：两个的中间用 `last`，三个以上的前面用 `between`，最后一个前面用 `last`。
fn listed(values: &[String], between: &str, last: &str) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., end] => format!("{}{last}{end}", init.join(between)),
    }
}

/// clap 的原话：它报错的第一行。
fn clap_said(error: &clap::Error) -> String {
    error
        .to_string()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests;
