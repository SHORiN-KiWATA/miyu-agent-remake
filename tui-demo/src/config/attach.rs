//! 附件认哪几种（`resources/attachments.json`），蓝图 `tui.md`「输入框」第 12 条。块上写的在 `text/zh.json` 的
//! `attachment_labels`，照种类的名字找。

use serde::Deserialize;

/// 认得的几种附件。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachLook {
    /// 每一种，照登记的先后。
    pub kinds: Vec<AttachKindLook>,
}

/// 一种附件。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachKindLook {
    /// `image`、`pdf`、`audio`、`video`：块上写的照它在 `attachment_labels` 里找。
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
        let labels = &config.text.attachment_labels;
        for kind in &config.attachments.kinds {
            let label = labels
                .get(&kind.name)
                .unwrap_or_else(|| panic!("{} 没有块上的字", kind.name));
            assert!(label.contains("{n}"), "{} 的块上要写第几个", kind.name);
            assert!(
                kind.extensions
                    .iter()
                    .all(|e| *e == e.to_ascii_lowercase() && !e.starts_with('.'))
            );
        }
        assert!(config.attachments.kinds.iter().any(|k| k.name == "image"));
    }
}
