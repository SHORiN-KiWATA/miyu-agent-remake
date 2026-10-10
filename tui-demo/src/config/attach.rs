//! 附件认哪几种、文件块上的名字最多几列（`resources/attachments.json`），蓝图 `tui.md`「输入框」第 12 条。块上写的
//! 在 `text/zh.json` 的 `attach`：附件照种类的名字找，文件块、目录块各一个写法。

use std::collections::HashMap;

use serde::Deserialize;

/// 认得的几种附件。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachLook {
    /// 文件块上的名字最多几列，多的中间截掉（一个汉字两列）。
    pub file_name_cols: usize,
    /// 每一种，照登记的先后。
    pub kinds: Vec<AttachKindLook>,
}

/// 块上写的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachTexts {
    /// 附件：种类的名字到块上写的，`{n}` 这一种的第几个。
    pub labels: HashMap<String, String>,
    /// 文件块：`{name}` 文件名。
    pub file: String,
    /// 目录块：`{name}` 目录名。
    pub folder: String,
    /// 每一种的名字（提示里用）：`image` 是「图片」。
    pub names: HashMap<String, String>,
    /// 模型收不了、改成了文件块时提示的（「输入框」第 12 条「照模型收」）：`{kinds}` 那几种的名字。
    pub refused: String,
}

/// 一种附件。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachKindLook {
    /// `image`、`pdf`、`audio`、`video`：块上写的照它在 `attach.labels` 里找。
    pub name: String,
    /// 认哪些扩展名：小写，不带点。
    pub extensions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::super::Config;

    #[test]
    fn every_kind_has_a_numbered_label_and_images_are_one_of_them() {
        // 剪贴板里的截图照 `image` 这一种收（「输入框」第 12 条）。
        let config = Config::builtin().unwrap();
        let labels = &config.text.attach.labels;
        for kind in &config.attachments.kinds {
            let label = labels
                .get(&kind.name)
                .unwrap_or_else(|| panic!("{} 没有块上的字", kind.name));
            assert!(label.contains("{n}"), "{} 的块上要写第几个", kind.name);
            assert!(
                config.text.attach.names.contains_key(&kind.name),
                "{} 没有名字",
                kind.name
            );
            assert!(
                kind.extensions
                    .iter()
                    .all(|e| *e == e.to_ascii_lowercase() && !e.starts_with('.'))
            );
        }
        assert!(config.attachments.kinds.iter().any(|k| k.name == "image"));
    }
}
