//! `miyu config set`、`unset`、`edit`、`trust` 给人看的字（施工 8-3，`docs/blueprint/config.md`「给人看的字」的「命令行」
//! 那张表、「样子」）。值由调用的一方写成 TOML 交进来（`"zh"`、`true`）。

use super::Language;

impl Language {
    /// 一层在句子里的叫法：`system`、`personal`、`project`、`env`，别的是默认值。英文带冠词，`capital` 的句首大写。
    fn layer_in(&self, layer: &str, capital: bool) -> String {
        let said = match (self, layer) {
            (Language::Chinese, "system") => "系统配置",
            (Language::Chinese, "personal") => "个人设置",
            (Language::Chinese, "project") => "项目配置",
            (Language::Chinese, "env") => "环境变量",
            (Language::Chinese, _) => "默认值",
            (Language::English, "system") => "the system config",
            (Language::English, "personal") => "personal settings",
            (Language::English, "project") => "the project config",
            (Language::English, "env") => "the environment",
            (Language::English, _) => "the default",
        };
        let mut chars = said.chars();
        match (capital, chars.next()) {
            (true, Some(first)) => first.to_uppercase().chain(chars).collect(),
            _ => said.to_string(),
        }
    }

    /// `set` 成了：`键 = 值` 写进了哪一层，什么时候生效（认不出的不说）。
    pub(crate) fn saved(&self, key: &str, value: &str, layer: &str, applies: &str) -> String {
        let layer = self.layer_in(layer, false);
        match (self, self.applies_after(applies)) {
            (Language::Chinese, Some(applies)) => {
                format!("· {key} = {value} 写进了{layer}，{applies}")
            }
            (Language::Chinese, None) => format!("· {key} = {value} 写进了{layer}"),
            (Language::English, Some(applies)) => {
                format!("· {key} = {value} saved to {layer}, {applies}")
            }
            (Language::English, None) => format!("· {key} = {value} saved to {layer}"),
        }
    }

    /// `set` 成了，可上面一层 `above` 压着：用的还是那一层写的 `effective`。
    pub(crate) fn saved_below(
        &self,
        key: &str,
        value: &str,
        layer: &str,
        above: &str,
        effective: &str,
    ) -> String {
        let (layer, says) = (self.layer_in(layer, false), self.layer_in(above, false));
        match self {
            Language::Chinese => format!(
                "· {key} = {value} 写进了{layer}，{says}里写着 {effective}，用的还是 {effective}"
            ),
            Language::English => {
                let verb = match above {
                    "personal" => "say",
                    _ => "says",
                };
                format!(
                    "· {key} = {value} saved to {layer}, but {says} {verb} {effective}, so {effective} stays in use"
                )
            }
        }
    }

    /// `set` 的值这一层本来就是。
    pub(crate) fn already(&self, value: &str) -> String {
        match self {
            Language::Chinese => format!("· 本来就是 {value}，没改"),
            Language::English => format!("· Already {value}, nothing changed"),
        }
    }

    /// `unset` 成了：从哪一层删掉了，现在是哪一层的什么。
    pub(crate) fn removed(&self, key: &str, layer: &str, effective: &str, from: &str) -> String {
        let (layer, from) = (self.layer_in(layer, false), self.layer_name(from));
        match self {
            Language::Chinese => format!("· 从{layer}里删掉了 {key}，现在是 {effective}（{from}）"),
            Language::English => {
                format!("· Removed {key} from {layer}. It is now {effective} ({from})")
            }
        }
    }

    /// `unset` 的这一层本来就没写。
    pub(crate) fn not_there(&self, key: &str, layer: &str) -> String {
        match self {
            Language::Chinese => format!("· {}里本来就没写 {key}", self.layer_in(layer, false)),
            Language::English => format!("· {} did not have {key}", self.layer_in(layer, true)),
        }
    }

    /// `set --project`：项目配置只能手改。
    pub(crate) fn project_by_hand(&self) -> &'static str {
        match self {
            Language::Chinese => "项目配置只能手改：miyu config edit --project",
            Language::English => "A project config is edited by hand: miyu config edit --project",
        }
    }

    /// `edit` 不在终端里。
    pub(crate) fn edit_needs_terminal(&self) -> &'static str {
        match self {
            Language::Chinese => "miyu config edit 要在终端里用",
            Language::English => "miyu config edit needs a terminal",
        }
    }

    /// `edit` 没改。
    pub(crate) fn nothing_changed(&self) -> &'static str {
        match self {
            Language::Chinese => "没改",
            Language::English => "Nothing changed",
        }
    }

    /// `edit` 有错，问接着改还是放弃：印在标准错误上，不换行，后面等人敲。
    pub(crate) fn edit_errors(&self, errors: usize) -> String {
        match (self, errors) {
            (Language::Chinese, n) => format!("有 {n} 处错误，还没存。回车接着改，输入 q 放弃："),
            (Language::English, 1) => {
                "1 error. Nothing saved yet. Press Enter to keep editing, or type q to give up: "
                    .to_string()
            }
            (Language::English, n) => format!(
                "{n} errors. Nothing saved yet. Press Enter to keep editing, or type q to give up: "
            ),
        }
    }

    /// `edit` 放弃了。
    pub(crate) fn gave_up(&self) -> &'static str {
        match self {
            Language::Chinese => "放弃了，文件没动",
            Language::English => "Gave up. The file is unchanged",
        }
    }

    /// `edit` 存的时候文件被别处改过了：改的留在副本 `copy` 里。
    pub(crate) fn edit_conflict(&self, copy: &str) -> String {
        match self {
            Language::Chinese => format!("你编辑的时候文件被改过了，没存。你改的在 {copy}"),
            Language::English => {
                format!(
                    "The file changed while you were editing. Nothing saved. Your edit is in {copy}"
                )
            }
        }
    }

    /// `edit` 的编辑器没有正常退出：退出码 `code`，被信号停掉的没有。
    pub(crate) fn editor_failed(&self, code: Option<i32>) -> String {
        let code = code.map_or_else(|| "-".to_string(), |code| code.to_string());
        match self {
            Language::Chinese => format!("编辑器没有正常退出（{code}），没存"),
            Language::English => format!("The editor did not exit cleanly ({code}). Nothing saved"),
        }
    }

    /// `edit` 存好了：改了的几项什么时候生效（照先后，一样的只说一次）；一项都没变的（只动了注释）不说，认不出的也不说。
    pub(crate) fn edit_saved(&self, applies: &[&str]) -> String {
        let said: Vec<String> = applies
            .iter()
            .filter_map(|applies| self.applies_after(applies))
            .collect();
        match (self, said.is_empty()) {
            (Language::Chinese, true) => "· 存好了".to_string(),
            (Language::English, true) => "· Saved".to_string(),
            (Language::Chinese, false) => format!("· 存好了，{}", said.join("、")),
            (Language::English, false) => format!("· Saved, {}", said.join(", ")),
        }
    }

    /// `trust` 这里没有项目配置。
    pub(crate) fn no_project_here(&self) -> &'static str {
        match self {
            Language::Chinese => "这里没有项目配置",
            Language::English => "There is no project config here",
        }
    }

    /// `trust` 列出它会改哪几项的第一行。
    pub(crate) fn would_set(&self, file: &str) -> String {
        match self {
            Language::Chinese => format!("{file} 会改这几项："),
            Language::English => format!("{file} would set:"),
        }
    }

    /// `trust` 问信不信任：印在标准错误上，不换行，后面等人敲。
    pub(crate) fn trust_question(&self) -> &'static str {
        match self {
            Language::Chinese => "项目配置只能让限制更严。信任这一份吗？[y/N] ",
            Language::English => {
                "A project config can only make limits stricter. Trust this one? [y/N] "
            }
        }
    }

    /// `trust` 不在终端里、又没写 `--yes`、`--no`。
    pub(crate) fn trust_needs_terminal(&self) -> &'static str {
        match self {
            Language::Chinese => "要在终端里回答，或者写 --yes、--no",
            Language::English => "Answer in a terminal, or pass --yes or --no",
        }
    }

    /// `trust` 本来就信任着这一份。
    pub(crate) fn already_trusted(&self) -> &'static str {
        match self {
            Language::Chinese => "已经信任过这一份了",
            Language::English => "This one is already trusted",
        }
    }

    /// `trust` 看的时候它又被改过了。
    pub(crate) fn trust_conflict(&self) -> &'static str {
        match self {
            Language::Chinese => "你看的时候它又被改过了，再跑一次",
            Language::English => "It changed while you were looking. Run this again",
        }
    }

    /// `trust` 记下了：信任、不信任。
    pub(crate) fn trust_answered(&self, trusted: bool) -> &'static str {
        match (self, trusted) {
            (Language::Chinese, true) => "· 信任了。内容变了会再问",
            (Language::Chinese, false) => "· 不信任，这一份不会用。内容变了会再问",
            (Language::English, true) => "· Trusted. You will be asked again if it changes",
            (Language::English, false) => {
                "· Not trusted. It will not be used. You will be asked again if it changes"
            }
        }
    }
}
