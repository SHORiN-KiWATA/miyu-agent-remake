//! 模型这一块的配置项（`docs/blueprint/models.md`「配置：模型这一块的键」，`config.md`「配置清单」，施工 8-6）：
//! 一家供应商 `[providers.<id>]`、一个模型手写的资料 `[providers.<id>.models."<model>"]`、用途 `[models]`。
//!
//! 8-6 只声明用得上的几格：驱动、地址、几个 key、对应目录里的哪一家、模型的窗口、主对话的模型。别的格（另配的头、倍率、
//! 缓存类别、开关、占位工具、本机、价格、挡位、池）随用到它的那一步加（「施工时定的」8-6）。项目配置一项都不能写。

use miyu_config::secret::Reference;

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
        /// 地址，路径由驱动接在后面。不写照档案推。
        base_url: Option<String> = none {
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
        /// 手写指定这一家对应目录里的哪一家。8-6 还没有目录：照它找档案、查模型资料。
        catalog: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
    }
}

miyu_config::settings! {
    /// 一个模型手写的资料（`models.md`「对外的样子」）：模型名是键里 `<model>` 那一段。8-6 只有窗口，取代开发用的
    /// `MIYU_DEV_WINDOW`。造会话、载入时交给内核，开着的会话不跟着变（限额会变随 8-10）。
    pub struct ModelSettings in "providers.<id>.models.<model>" {
        /// 上下文窗口，单位 token。
        window: Option<i64> = none {
            kind: int [1, 100000000],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "providers", control: number },
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
