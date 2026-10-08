//! `miyu setup` 给人看的字（施工 8-11，`docs/blueprint/cli/setup.md`「给人看的字」）。key 从不出现在这里的任何一句里。
//! 「没选」「没收到 key」「存好了」和贴 key 那一句照 `miyu login` 的（`language/login.rs`）。

use super::Language;

impl Language {
    /// 用不了：`after` 的接在「在哪」后面，带开头的逗号。
    pub(crate) fn unusable(&self, why: &str, after: bool) -> String {
        match (self, after) {
            (Language::Chinese, true) => format!("，用不了：{why}"),
            (Language::Chinese, false) => format!("用不了：{why}"),
            (Language::English, true) => format!(", cannot use: {why}"),
            (Language::English, false) => format!("cannot use: {why}"),
        }
    }

    /// 为什么用不了：认不出接口、没有地址、驱动还没有（`driver` 是它的写法）。
    pub(crate) fn unusable_why(&self, driver: Option<&str>, address: bool) -> String {
        match (self, driver, address) {
            (Language::Chinese, None, _) => "认不出它的接口".to_string(),
            (Language::Chinese, Some(_), false) => "目录里没有它的地址".to_string(),
            (Language::Chinese, Some(driver), true) => format!("还没有 {driver} 驱动"),
            (Language::English, None, _) => "its API is not known".to_string(),
            (Language::English, Some(_), false) => "the catalog has no address for it".to_string(),
            (Language::English, Some(driver), true) => format!("no {driver} driver yet"),
        }
    }

    /// 问编号。
    pub(crate) fn pick_number(&self) -> &'static str {
        match self {
            Language::Chinese => "选一个编号：",
            Language::English => "Pick a number: ",
        }
    }

    /// 敲的不是列出的编号。
    pub(crate) fn not_a_number(&self, typed: &str) -> String {
        match self {
            Language::Chinese => format!("{typed} 不是列出的编号"),
            Language::English => format!("{typed} is not a listed number"),
        }
    }

    /// 头看得到、核心看不到的变量：几个名字连起来。
    pub(crate) fn core_cannot_see(&self, names: &[&str]) -> String {
        match self {
            Language::Chinese => format!(
                "· 这个终端里设了 {}，核心看不到：核心是别处拉起的，看不到后来设的环境变量。等核心空闲了自己退出（没有界面连着、没有在跑的活），再在这个终端里运行 miyu setup；或者选这一家、把 key 贴进来。",
                names.join("、")
            ),
            Language::English => format!(
                "· {} is set in this terminal, but the core cannot see it: the core was started elsewhere and does not see variables set later. Wait until the core is idle and exits by itself (no interface connected, nothing running), then run miyu setup in this terminal again; or pick that provider and paste the key.",
                names.join(", ")
            ),
        }
    }

    /// `--provider` 不是目录里能用的一家。
    pub(crate) fn no_usable_provider(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("目录里没有能用的 {id}"),
            Language::English => format!("No usable provider {id} in the catalog"),
        }
    }

    /// 不在终端里又没写 `--provider`。
    pub(crate) fn setup_needs_terminal(&self) -> &'static str {
        match self {
            Language::Chinese => "要在终端里选，或者写 miyu setup --provider <编号>",
            Language::English => "Pick in a terminal, or run miyu setup --provider <id>",
        }
    }

    /// 试之前。
    pub(crate) fn trying(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("试一下 {name}……"),
            Language::English => format!("Trying {name}…"),
        }
    }

    /// 通了。
    pub(crate) fn it_works(&self, model: &str, ms: u64) -> String {
        match self {
            Language::Chinese => format!("· 通了：试的 {model}，{ms} 毫秒收到第一个字。"),
            Language::English => format!("· It works: tried {model}, first token in {ms} ms."),
        }
    }

    /// 模型照目录列的。
    pub(crate) fn listed_from_catalog(&self) -> &'static str {
        match self {
            Language::Chinese => "· 供应商列不出模型，下面照 models.dev 的目录列。",
            Language::English => {
                "· The provider listed no models; the list below is from the models.dev catalog."
            }
        }
    }

    /// 不通：哪一步、分类、原话（原话里有 HTTP 状态的照原话，和 `miyu ask` 出错那一行一样）。
    pub(crate) fn did_not_work(&self, stage: &str, class: &str, message: &str) -> String {
        let stage = match (self, stage) {
            (Language::Chinese, "config") => "配置",
            (Language::Chinese, "list") => "列模型",
            (Language::Chinese, _) => "发请求",
            (Language::English, "config") => "config",
            (Language::English, "list") => "listing models",
            (Language::English, _) => "request",
        };
        let class = self.class_name(class);
        let mut said = match self {
            Language::Chinese => format!("不通（{stage}）：{class}"),
            Language::English => format!("Did not work ({stage}): {class}"),
        };
        let message = message.trim();
        if !message.is_empty() {
            said.push_str(match self {
                Language::Chinese => "：",
                Language::English => ": ",
            });
            said.push_str(message);
        }
        said
    }

    /// 选模型的头一行。
    pub(crate) fn models_heading(&self) -> &'static str {
        match self {
            Language::Chinese => "选主对话的模型：",
            Language::English => "Pick the model for chat:",
        }
    }

    /// 推荐的，接在模型名后面。
    pub(crate) fn recommended(&self) -> &'static str {
        match self {
            Language::Chinese => "（推荐）",
            Language::English => " (recommended)",
        }
    }

    /// 还有几个没列。
    pub(crate) fn more_models(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("还有 {count} 个，敲名字也行。"),
            Language::English => format!("{count} more; you can type a name."),
        }
    }

    /// 问模型。
    pub(crate) fn pick_model(&self) -> &'static str {
        match self {
            Language::Chinese => "选一个编号，直接回车用推荐的：",
            Language::English => "Pick a number, or press Enter for the recommended one: ",
        }
    }

    /// 敲的不在列表里。
    pub(crate) fn not_listed(&self, typed: &str) -> String {
        match self {
            Language::Chinese => format!("{typed} 不在列表里"),
            Language::English => format!("{typed} is not in the list"),
        }
    }

    /// 写好了。
    pub(crate) fn set_up(&self, reference: &str) -> String {
        match self {
            Language::Chinese => format!("写好了：models.chat = {reference}"),
            Language::English => format!("Done: models.chat = {reference}"),
        }
    }

    /// `miyu ask` 没模型，先走 setup。
    pub(crate) fn setup_first(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有模型，先接上一个。",
            Language::English => "No model is set up yet. Let's connect one first.",
        }
    }

    /// 选一家那张表的头一行（施工 8-11 再补）。
    pub(crate) fn pick_provider(&self) -> &'static str {
        match self {
            Language::Chinese => "选一家：",
            Language::English => "Pick a provider:",
        }
    }

    /// 环境变量里找到了这一家的 key（施工 8-11 再补）。
    pub(crate) fn key_found(&self) -> &'static str {
        match self {
            Language::Chinese => "已找到 key",
            Language::English => "key found",
        }
    }

    /// 这一家配置里已经有了（施工 8-11 再补）。
    pub(crate) fn set_up_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "已配好",
            Language::English => "set up",
        }
    }

    /// 本机跑着的服务（施工 8-11 再补）。
    pub(crate) fn local_mark(&self, base_url: &str) -> String {
        match self {
            Language::Chinese => format!("本机 {base_url}"),
            Language::English => format!("local {base_url}"),
        }
    }

    /// 最后一行：自定义（施工 8-11 再补）。
    pub(crate) fn custom(&self) -> &'static str {
        match self {
            Language::Chinese => "自定义",
            Language::English => "Custom",
        }
    }

    /// 问自定义的地址。
    pub(crate) fn base_url(&self) -> &'static str {
        match self {
            Language::Chinese => "Base URL：",
            Language::English => "Base URL: ",
        }
    }

    /// 地址不是 http、https 的。
    pub(crate) fn not_a_url(&self) -> &'static str {
        match self {
            Language::Chinese => "要以 http:// 或 https:// 开头",
            Language::English => "Must start with http:// or https://",
        }
    }

    /// 接口协议那张表的头一行。
    pub(crate) fn protocol_heading(&self) -> &'static str {
        match self {
            Language::Chinese => "接口协议：",
            Language::English => "API protocol:",
        }
    }

    /// 一种接口协议叫什么：驱动的写法 `driver`。
    pub(crate) fn protocol(&self, driver: &str) -> &'static str {
        match (self, driver) {
            (Language::Chinese, "openai-chat") => "OpenAI 兼容",
            (Language::English, "openai-chat") => "OpenAI compatible",
            (_, "anthropic") => "Anthropic",
            _ => "OpenAI Responses",
        }
    }

    /// 问自定义的 key：可以空。
    pub(crate) fn paste_key_optional(&self) -> &'static str {
        match self {
            Language::Chinese => "Key（不显示，可以空）：",
            Language::English => "Key (hidden, may be empty): ",
        }
    }

    /// 取不到模型列表（2026-10-08 项目主人定的说法）。
    pub(crate) fn no_model_list(&self) -> &'static str {
        match self {
            Language::Chinese => "未获取到模型列表",
            Language::English => "Could not get the model list",
        }
    }

    /// 问模型名。
    pub(crate) fn model_name(&self) -> &'static str {
        match self {
            Language::Chinese => "模型名：",
            Language::English => "Model name: ",
        }
    }
}
