//! 一条问题说成给人看的话（`docs/blueprint/config.md`「怎么走」第四条、「给人看的字」的「报错的话」）：错在哪（期望、
//! 收到），改法，现在照什么用着。字都在资源的 `said` 里（`config/…`），照 [`Words`] 那一种语言。
//!
//! 一句由几段接成：每一段不是以句末的标点（`config/stops` 里的几个字）结尾的，照 `config/sentence` 补上句号；段和段
//! 照 `config/then` 接。中文「是不是想写 ui.language？」后面不再补「。」，英文段和段之间空一格，都是这几句定的，代码
//! 里不写标点。

use crate::item::{Item, Kind};
use crate::merge::Origin;
use crate::problem::{Code, Problem};
use crate::value::Value;
use crate::words::{self, Missing, Words, one_of, sentence};

/// 现在照什么用着（「报错」的 `using`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Using {
    /// 一项的问题：丢掉这一项以后，下面几层合出来的值和它从哪来。
    Value(Value, Origin),
    /// 整份文件的问题：照上一次读好的那一份用。
    LastGood,
    /// 整份文件的问题：起来时就读不好，这份文件照空的。
    Nothing,
}

/// 说成话的一条问题：`expected` 期望什么（有的才有），`message` 整句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Told {
    /// 期望什么，给人看的。
    pub expected: Option<String>,
    /// 整句给人看的话。
    pub message: String,
}

/// 把 `problem` 说成话，项照清单 `items` 查，现在照什么用着是 `using`（没有的不说）。
///
/// # Errors
///
/// 要用的字缺了（资源目录装得不全）。
pub fn tell(
    problem: &Problem,
    items: &[Item],
    using: Option<&Using>,
    words: &dyn Words,
) -> Result<Told, Missing> {
    let key = problem.key.as_deref().unwrap_or_default();
    let got = problem.got.as_deref().unwrap_or_default();
    let why = problem.why.as_deref().unwrap_or_default();
    let item = crate::key::item_of(items, key);
    let mut expected = None;
    let mut parts = Vec::new();
    let mut says_using = problem.code.whole_file();
    match problem.code {
        Code::Unreadable => parts.push(sentence(words, "config/unreadable", &[("why", why)])?),
        Code::TooBig => parts.push(sentence(words, "config/too-big", &[])?),
        Code::NotUtf8 => parts.push(sentence(words, "config/not-utf8", &[])?),
        Code::Syntax => parts.push(sentence(words, "config/syntax", &[("why", why)])?),
        Code::UnknownKey => {
            parts.push(match problem.suggest {
                Some(suggest) => sentence(
                    words,
                    "config/unknown-key",
                    &[("key", key), ("suggest", suggest)],
                )?,
                None => sentence(words, "config/unknown-key-plain", &[("key", key)])?,
            });
            // 查询里写错的键不在哪一行：没有「这一行先不管」。
            if problem.at.is_some() {
                parts.push(sentence(words, "config/kept", &[])?);
            }
        }
        Code::WrongType | Code::NotAnOption | Code::OutOfRange | Code::BadFormat => match item {
            Some(item) => {
                says_using = true;
                let (said, fix) = value_problem(item, key, problem.code, got, words)?;
                expected = Some(said.0);
                parts.push(said.1);
                parts.extend(Some(fix).filter(|fix| !fix.is_empty()));
            }
            None => {
                let table = sentence(words, "config/expected/table", &[])?;
                parts.push(sentence(
                    words,
                    "config/wrong-type",
                    &[("key", key), ("expected", &table), ("got", got)],
                )?);
                expected = Some(table);
            }
        },
        Code::WrongLayer => {
            let layers = layer_names(item.map_or(&[][..], |item| item.layers), words)?;
            let layer = sentence(
                words,
                &format!("config/layer/{}", problem.layer.as_str()),
                &[],
            )?;
            parts.push(sentence(
                words,
                "config/wrong-layer",
                &[("key", key), ("layers", &layers), ("layer", &layer)],
            )?);
            parts.push(sentence(words, "config/fix-move", &[("layers", &layers)])?);
        }
        Code::NotTightening => {
            let current = problem
                .current
                .as_ref()
                .map(Value::toml)
                .unwrap_or_default();
            parts.push(sentence(
                words,
                "config/not-tightening",
                &[("key", key), ("current", &current), ("got", got)],
            )?);
        }
        Code::UntrustedProject => parts.push(sentence(words, "config/untrusted-project", &[])?),
        Code::UnknownSecret | Code::EnvNotSet => {
            let said = match problem.code {
                Code::UnknownSecret => "config/unknown-secret",
                _ => "config/env-not-set",
            };
            let name = problem.name.as_deref().unwrap_or_default();
            parts.push(sentence(words, said, &[("key", key), ("name", name)])?);
        }
        Code::BadSegment => {
            let name = problem.name.as_deref().unwrap_or_default();
            let expected = match why {
                crate::key::MODEL => "config/expected/model-name",
                _ => "config/expected/id",
            };
            let expected = sentence(words, expected, &[])?;
            parts.push(sentence(
                words,
                "config/bad-segment",
                &[("key", key), ("name", name), ("expected", &expected)],
            )?);
        }
        Code::SecretName => parts.push(sentence(words, "config/bad-secret-name", &[("key", key)])?),
        Code::SecretValue => {
            parts.push(sentence(words, "config/bad-secret-value", &[("key", key)])?);
        }
    }
    if let Some(using) = using.filter(|_| says_using) {
        parts.push(using_said(using, words)?);
    }
    Ok(Told {
        expected,
        message: joined(&parts, words)?,
    })
}

/// 值写得不对的一项（真的键 `key`）：（期望什么，错在哪那一段）和改法。选项列出能写的几个，改法取默认值（没有默认值的
/// 取第一个）；开关期望 `true` 或 `false`，改法取默认值的另一个（「怎么走」第四条第 5 条）；数不在范围里的说范围；别的
/// 类型期望照 [`words::expected`]，不另说改法（施工 8-6）。
fn value_problem(
    item: &Item,
    key: &str,
    code: Code,
    got: &str,
    words: &dyn Words,
) -> Result<((String, String), String), Missing> {
    match item.kind {
        Kind::Option(options) => {
            let listed = one_of(words, options, "config/or-values")?;
            let said = sentence(
                words,
                "config/not-an-option",
                &[("key", key), ("options", &listed), ("got", got)],
            )?;
            let example = match &item.default {
                Some(default) => default.toml(),
                None => Value::Text(options.first().copied().unwrap_or_default().into()).toml(),
            };
            let example = format!("{key} = {example}");
            let fix = sentence(words, "config/fix-example", &[("example", &example)])?;
            Ok(((listed, said), fix))
        }
        Kind::Bool => {
            let expected = sentence(words, "config/expected/bool", &[])?;
            let said = sentence(
                words,
                "config/wrong-type",
                &[("key", key), ("expected", &expected), ("got", got)],
            )?;
            let on = item.default.as_ref().is_some_and(bool::from);
            let example = format!("{key} = {}", Value::Bool(!on).toml());
            let fix = sentence(words, "config/fix-write", &[("example", &example)])?;
            Ok(((expected, said), fix))
        }
        Kind::Int { min, max } if code == Code::OutOfRange => {
            let expected = words::expected(words, item.kind)?;
            let said = sentence(
                words,
                "config/out-of-range",
                &[
                    ("key", key),
                    ("min", &min.to_string()),
                    ("max", &max.to_string()),
                    ("got", got),
                ],
            )?;
            Ok(((expected, said), String::new()))
        }
        kind => {
            let expected = words::expected(words, kind)?;
            let said = match code {
                Code::BadFormat => "config/bad-format",
                _ => "config/wrong-type",
            };
            let said = sentence(
                words,
                said,
                &[("key", key), ("expected", &expected), ("got", got)],
            )?;
            // 能写的已经在期望里了，不另说改法。
            Ok(((expected, said), String::new()))
        }
    }
}

/// 几层的名字连成「系统配置或个人设置」。
fn layer_names(layers: &[crate::item::Layer], words: &dyn Words) -> Result<String, Missing> {
    let mut names = Vec::new();
    for layer in layers {
        names.push(sentence(
            words,
            &format!("config/layer/{}", layer.as_str()),
            &[],
        )?);
    }
    one_of(
        words,
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
        "config/or",
    )
}

/// 「现在照什么用着」那一段。
fn using_said(using: &Using, words: &dyn Words) -> Result<String, Missing> {
    match using {
        Using::Value(value, origin) => {
            let from = sentence(words, &format!("config/layer/{}", origin.layer_name()), &[])?;
            sentence(
                words,
                "config/using-value",
                &[("value", &value.toml()), ("from", &from)],
            )
        }
        Using::LastGood => sentence(words, "config/using-last-good", &[]),
        Using::Nothing => sentence(words, "config/using-nothing", &[]),
    }
}

/// 几段接成一句：没以句末的标点结尾的补上句号，段和段照 `config/then` 接。
fn joined(parts: &[String], words: &dyn Words) -> Result<String, Missing> {
    let stops = sentence(words, "config/stops", &[])?;
    let mut message: Option<String> = None;
    for part in parts {
        let ended = part.ends_with(|c: char| stops.contains(c));
        let part = match ended {
            true => part.clone(),
            false => sentence(words, "config/sentence", &[("text", part)])?,
        };
        message = Some(match message {
            None => part,
            Some(rest) => {
                words::sentence(words, "config/then", &[("rest", &rest), ("next", &part)])?
            }
        });
    }
    Ok(message.unwrap_or_default())
}
