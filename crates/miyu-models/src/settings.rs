//! 模型这一块的配置项（`docs/blueprint/models.md`「配置：模型这一块的键」，`config.md`「配置清单」，施工 8-6）：
//! 一家供应商 `[providers.<id>]`、一个模型手写的资料 `[providers.<id>.models."<model>"]`、用途 `[models]`。
//!
//! 8-6 只声明用得上的几格：驱动、地址、几个 key、对应目录里的哪一家、模型的窗口、主对话的模型。8-7 加上模型资料要的
//! （`models.md`「模型的资料」）：供应商的倍率、本机；模型手写的资料（对目录里的哪一个、最大输出、能收什么、能不能调工具、
//! 思考强度、价格、倍率）；目录怎么更新 `[models.catalog]`。别的格（另配的头、缓存类别、开关、占位工具、模型的驱动、挡位、
//! 池）随用到它的那一步加（「施工时定的」8-6、8-7）。项目配置一项都不能写。
//!
//! `base_url` 8-6b 起也能写 `{ env = … }`：地址不进任何回应、日志、文件，照核心起来时的环境取（[`crate::provider`] 的
//! `resolve_base_url`）。

use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_config::{Address, Number};

miyu_config::settings! {
    /// 一家供应商（`models.md`「怎么走」第一条）：编号是键里 `<id>` 那一段，「路径里的名字」的写法。
    pub struct ProviderSettings in "providers.<id>" {
        /// 怎么说话。不写照档案推；推不出来的这一家用不了。
        driver: Option<String> = none {
            kind: option ["openai-chat", "anthropic", "openai-responses"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: select },
        },
        /// 地址，路径由驱动接在后面。不写照档案推。可以是写死的，也可以是一个环境变量的引用（`{ env = … }`，施工
        /// 8-6b）：本机端点地址和 key 一样，只想放在拉起核心的环境变量里，不进任何文件。
        base_url: Option<Address> = none {
            kind: url,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
        /// 几个 key：`{ secret = … }` 或 `{ env = … }`。一个会话钉在其中一个上（[`crate::keys`]）。空的不带认证头。
        keys: Vec<Reference> = [] {
            kind: secrets,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: list },
        },
        /// 手写指定这一家对应目录里的哪一家：照它找档案、认目录（`models.md`「怎么走」第二条第 4 条第 2 层）。
        catalog: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
        /// 倍率：价格照它乘，模型上写的盖过它（第二条第 11 条，施工 8-7）。不写是 1。
        price_multiplier: Option<Number> = none {
            kind: float [0, 1000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 本机的模型服务：价格当 0（第二条第 12 条，施工 8-7）。不写的照地址：在本机的是。
        local: Option<bool> = none {
            kind: bool,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: toggle },
        },
    }
}

miyu_config::settings! {
    /// 一个模型手写的资料（`models.md`「对外的样子」）：模型名是键里 `<model>` 那一段。每一格都压过用出来的、供应商的列表、
    /// 目录（「模型的资料」那张表）。窗口、最大输出造会话、载入时交给内核，开着的会话不跟着变（限额会变随 8-10）。
    pub struct ModelSettings in "providers.<id>.models.<model>" {
        /// 上下文窗口，单位 token。
        window: Option<i64> = none {
            kind: int [1, 100000000],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 手写指定照目录里的哪一个：`<目录里的供应商>/<目录里的模型>`（第二条第 4 条第 1 层，施工 8-7）。
        catalog: Option<String> = none {
            kind: text [256],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
        /// 最大输出，单位 token（施工 8-7）。
        max_output: Option<i64> = none {
            kind: int [1, 100000000],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 能收哪些输入（施工 8-7）。
        inputs: Option<Vec<String>> = none {
            kind: options ["text", "image", "pdf"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: list },
        },
        /// 能不能调工具（施工 8-7）：只给 `model.list` 看。
        tools: Option<bool> = none {
            kind: bool,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: toggle },
        },
        /// 思考强度有哪几级（施工 8-7）：只给 `model.list` 看。
        reasoning: Option<Vec<String>> = none {
            kind: texts [32],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: list },
        },
        /// 倍率：盖过供应商上写的（施工 8-7）。
        price_multiplier: Option<Number> = none {
            kind: float [0, 1000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
    }
}

miyu_config::settings! {
    /// 一个模型手写的价格（施工 8-7）：每一百万 token 的价。写了一格就整份用手写的，不和目录的拼（「模型的资料」第一条）。
    pub struct PriceSettings in "providers.<id>.models.<model>.price" {
        /// 输入。
        input: Option<Number> = none {
            kind: float [0, 1000000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 输出。
        output: Option<Number> = none {
            kind: float [0, 1000000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 读缓存。
        cache_read: Option<Number> = none {
            kind: float [0, 1000000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 写缓存。
        cache_write: Option<Number> = none {
            kind: float [0, 1000000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 币种：ISO 4217 的三个字母。不写是 `USD`。
        currency: Option<String> = none {
            kind: text [3],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
    }
}

miyu_config::settings! {
    /// models.dev 的目录怎么更新（`models.md`「对外的样子」`[models.catalog]`、「怎么走」第二条第 3 条，施工 8-7）：当场生效。
    pub struct CatalogSettings in "models.catalog" {
        /// 后台去拉新的。关掉只用安装包带的和缓存里已有的。环境变量 `MIYU_CATALOG_UPDATE` 压过（离线的机器、测试拉起的核心）。
        update: bool = true {
            kind: bool,
            layers: [System, Personal],
            env: "MIYU_CATALOG_UPDATE",
            applies: now,
            ui: { page: "models", group: "catalog", control: toggle },
        },
        /// 从哪拉。
        url: String = "https://models.dev/api.json" {
            kind: url,
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "catalog", control: text },
        },
        /// 缓存旧过这么久才拉。
        every: Duration = "24h" {
            kind: duration [3600, 2592000],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "catalog", control: text },
        },
    }
}

miyu_config::settings! {
    /// 用途（`models.md`「对外的样子」`[models]`）。
    pub struct UseSettings in "models" {
        /// 新会话默认用的模型：`<供应商>/<模型>`。没配的请求都是 `no_model`。
        chat: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "uses", common: true, control: text },
        },
    }
}
