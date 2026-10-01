//! 模型这一块的配置项（`docs/blueprint/models.md`「配置：模型这一块的键」，`config.md`「配置清单」，施工 8-6）：
//! 一家供应商 `[providers.<id>]`、一个模型手写的资料 `[providers.<id>.models."<model>"]`、用途 `[models]`、池
//! `[pools.<名字>]`。
//!
//! 8-6 只声明用得上的几格：驱动、地址、几个 key、对应目录里的哪一家、模型的窗口、主对话的模型。8-7 加上模型资料要的
//! （`models.md`「模型的资料」）：供应商的倍率、本机；模型手写的资料（对目录里的哪一个、最大输出、能收什么、能不能调工具、
//! 思考强度、价格、倍率）；目录怎么更新 `[models.catalog]`。8-8 加上看图的模型、池，供应商的缓存类别（池不写分法时照它
//! 定，[`crate::pools`]）；8-8 的四个挡位 8-8 补去掉了，池多派子代理能不能选、给模型看的说明两项。别的格（另配的头、开关、占位工具、模型的驱动）随用到它的那一步加（「施工时定的」
//! 8-6、8-7、8-8）。8-9 加上冷却 `[models.cooldown]` 的三类。8-18 加上模型默认的思考强度
//! `effort`。项目配置一项都不能写。
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
        /// 缓存属于哪一类（`08-上下文投影.md` 第六节，施工 8-8）：现在只用来定池不写分法时怎么分——成员全是按次计费的
        /// （`per_request`）轮换，别的钉住（[`crate::pools`]）。不写照驱动的默认，没有哪种驱动默认按次计费。
        cache: Option<String> = none {
            kind: option ["contract", "best_effort", "per_request"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: select },
        },
    }
}

miyu_config::settings! {
    /// 一个模型手写的资料（`models.md`「对外的样子」）：模型名是键里 `<model>` 那一段。每一格都压过用出来的、供应商的列表、
    /// 目录（「模型的资料」那张表）。窗口、最大输出造会话、载入时交给内核，开着的会话下一个回合开始时跟着换（施工 8-10）。
    pub struct ModelSettings in "providers.<id>.models.<model>" {
        /// 上下文窗口，单位 token。
        window: Option<i64> = none {
            kind: int [1, 100000000],
            layers: [System, Personal],
            applies: next_turn,
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
            applies: next_turn,
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
        /// 思考强度有哪几档（施工 8-7）：盖过目录的；`none`、`disabled` 读成 `off`（施工 8-18，[`crate::effort`]）。
        reasoning: Option<Vec<String>> = none {
            kind: texts [32],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: list },
        },
        /// 默认的思考强度（施工 8-18，`models.md`「怎么走」第十一条第 2 条）：这个模型的一档，不写的请求里不带、照供应商的
        /// 默认。不在这时的档位里的照没写，配置报 `unknown_effort`（[`crate::effort::unknown`]）。
        effort: Option<String> = none {
            kind: text [32],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
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
        /// 从哪拉。也能写 `{ env = … }`（施工 8-8：网址类型整体认引用，8-6b 留下的这一项以前读成空的），照核心的环境取。
        url: Address = "https://models.dev/api.json" {
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
        /// 新会话默认用的模型：`<供应商>/<模型>` 或 `@<池>`。没配的请求都是 `no_model`。
        chat: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "uses", common: true, control: text },
        },
        /// 替看不了图的模型看图的模型（施工 8-8 只读进来、`model.list` 列出来；替看图随 8-17）。
        vision: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "uses", control: text },
        },
    }
}

miyu_config::settings! {
    /// 一个池（`models.md`「对外的样子」`[pools.<名字>]`，「怎么走」第三条第 6 条，施工 8-8）：几个模型编成一组。名字是键里
    /// `<id>` 那一段，「路径里的名字」的写法。
    pub struct PoolSettings in "pools.<id>" {
        /// 成员：模型的列表，只能是 `<供应商>/<模型>`，照写的先后。可以是空的（`miyu setup` 预先建的三个池），一个都没有的池
        /// 解析不出。
        models: Option<Vec<String>> = none {
            kind: models,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "pools", control: list },
        },
        /// 怎么分：`pin` 钉住（一个会话一直用一个成员），`rotate` 轮换（每次请求换下一个）。不写的照成员的缓存类别定。
        strategy: Option<String> = none {
            kind: option ["pin", "rotate"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "pools", control: select },
        },
        /// 在不在派子代理的选项里（施工 8-8 补，`models.md`「工具」）：开着、有认得出的成员的，新会话的 `subagent` 能选它
        /// （[`crate::pools::offered`]）。会话开局时拼进工具面，整个会话不变：新会话生效。
        subagent: bool = false {
            kind: bool,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "pools", control: toggle },
        },
        /// 给模型看的一句（施工 8-8 补）：`subagent` 的参数里接在池名后面。一行英文，CJK 的字占一半以上的不收。
        description: Option<String> = none {
            kind: english [60],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "pools", control: text },
        },
    }
}

miyu_config::settings! {
    /// 限速的冷却（`models.md`「对外的样子」`[models.cooldown]`、「怎么走」第五条第 2 条，施工 8-9）：连着失败的第 n 次冷却
    /// `min(base × 2^(n−1), max)`，供应商说得更长的照它、也不超过 `max`。当场生效，下一次出错用新的。
    pub struct RateLimitedCooldown in "models.cooldown.rate_limited" {
        /// 第一次冷却多久。
        base: Duration = "30s" {
            kind: duration [1, 3600],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "cooldown", control: text },
        },
        /// 最多冷却多久。
        max: Duration = "10m" {
            kind: duration [1, 86400],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "cooldown", control: text },
        },
    }
}

miyu_config::settings! {
    /// 可重试的错（连不上、5xx）的冷却（施工 8-9）：算法同 [`RateLimitedCooldown`]。
    pub struct RetryableCooldown in "models.cooldown.retryable" {
        /// 第一次冷却多久。
        base: Duration = "10s" {
            kind: duration [1, 3600],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "cooldown", control: text },
        },
        /// 最多冷却多久。
        max: Duration = "5m" {
            kind: duration [1, 86400],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "cooldown", control: text },
        },
    }
}

miyu_config::settings! {
    /// 认证失败（额度用完的也在这一类）的冷却（施工 8-9）：停整个 key，算法同 [`RateLimitedCooldown`]。
    pub struct AuthCooldown in "models.cooldown.auth" {
        /// 第一次冷却多久。
        base: Duration = "10m" {
            kind: duration [1, 3600],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "cooldown", control: text },
        },
        /// 最多冷却多久。
        max: Duration = "2h" {
            kind: duration [1, 86400],
            layers: [System, Personal],
            applies: now,
            ui: { page: "models", group: "cooldown", control: text },
        },
    }
}
