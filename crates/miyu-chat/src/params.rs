//! 出厂参数和按场所改（`docs/blueprint/chat.md` 第八条，`docs/designs/18-通讯平台.md` Q26，施工 O-15）。
//!
//! 第二到第六条要的数有一份出厂的数据文件（软件包资源里的 `defaults.toml`）：[`Params::read`] 照声明读、照声明查，有一条
//! 问题就整份不用；场所规则写同名的表改的几项，由 [`Params::at`] 套上。表的名字和每一项的类型只在 `items` 里声明一次，
//! 出厂文件、场所规则都照它查（第七条第 5 条）；读表的那一份在 `rules/tables.rs`，场所规则和出厂文件共用。
//!
//! 纯逻辑：读文件、出厂文件有问题时怎么报，由桥管。[`Base64`]、[`Chatty`]、[`Window`]、[`Restraint`]、[`Outbound`] 的格只在
//! crate 里可见，外面造不出，只能从这里拿，拿到的都照声明查过（自查第 14 条：半衰期是 0 会算出 NaN、门槛归零）。
//!
//! [`Window`]: crate::Window
//! [`Restraint`]: crate::Restraint

pub(crate) mod items;

use crate::rules::{File, Problem, Resolved};
use crate::{Base64, Chatty, Judge, Outbound, Restraint, Window};

/// 第二到第六条要的参数（第八条「对外的样子」）：出厂文件读出来（[`Params::read`]），再套上场所规则改的几项
/// （[`Params::at`]）。
///
/// 格公开，桥照它读：要守住的几样（[`Base64`]、[`Chatty`]、[`Outbound`]）格收在 crate 里，外面造不出；顶替窗口、拆段的
/// 长度、判官的几项是桥用的，或者交给只收数的函数（第八条施工时定的第 14 条）。
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// 进站链：正文里的 base64 怎么解、怎么筛（`[inbound]`）。
    pub base64: Base64,
    /// 主动回复判断（`[chatty]`）。
    pub chatty: Chatty,
    /// 顶替窗口，毫秒（`[dispatch]` 的 `supersede_window`）：交给 [`supersede()`](crate::supersede())。
    pub supersede_window: i64,
    /// 判官的几项（`[judge]`）。
    pub judge: Judge,
    /// 出站链：引用、@、去重（`[outbound]`）。
    pub outbound: Outbound,
    /// 按段拆开时一段最多几个字符（`[outbound]` 的 `split_chars`）：交给 [`split()`](crate::split())，`0` 是不拆。
    pub split_chars: usize,
}

impl Params {
    /// 读出厂文件（第八条「怎么走」第 2 条）：最上面只认声明里的表，每一项照声明读、照声明查，每一项都得写
    /// （`judge.model` 除外）。
    ///
    /// 收第一条的 [`File`]，问题照它说是哪个文件，和场所规则的问题一个样（施工时定的第 8 条）。读文件、文件大小、是不是
    /// UTF-8 由读文件的一方管。
    ///
    /// # Errors
    ///
    /// 有一条问题就整份不用，交回全部问题（[`Problem`]，没有第几条规则，键是 `表.项`）：TOML 写法不对 `syntax`；不认识的
    /// 表、项 `unknown_key`（警告也算：出厂文件是打包的，有问题是打包的错，施工时定的第 10 条）；值写错照配置的原因码；
    /// 缺了的 `wrong_type`，没有位置（施工时定的第 9 条）。
    pub fn read(file: &File) -> Result<Params, Vec<Problem>> {
        let mut params = blank();
        // 缺了的已经报过：读得出来，声明里该写的每一项都写了，每一格都填过。
        for (item, value) in crate::rules::defaults(file)? {
            item.put(&mut params, &value);
        }
        Ok(params)
    }

    /// 套上场所规则改的几项（第八条「怎么走」第 5 条）：从这一份起，`resolved` 里键是 `表.项` 的，照声明再查一遍，合的
    /// 换上去；别的属性（`persona`、`rate`……）不看。
    ///
    /// 不合声明的跳过：[`Rules::resolve`](crate::Rules::resolve) 交出的都查过，只有手造的 [`Resolved`] 里会有，照配置
    /// `Setting` 的先例照「没有」读（施工时定的第 13 条）。
    pub fn at(&self, resolved: &Resolved) -> Params {
        let mut params = self.clone();
        for (key, entry) in &resolved.entries {
            if let Some(item) = items::find(key)
                && item.check(&entry.value).is_ok()
            {
                item.put(&mut params, &entry.value);
            }
        }
        params
    }
}

/// 一份全是 0 的参数：[`Params::read`] 从它起填。不出这个模块：有几格（`restraint_k` 是 0）照它算会算坏（施工时定的
/// 第 12 条）。
fn blank() -> Params {
    let window = Window {
        bonus: 0.0,
        window: 0,
    };
    Params {
        base64: Base64 {
            min_chars: 0,
            max_chars: 0,
            printable: 0,
        },
        chatty: Chatty {
            probability: 0,
            base: 0.0,
            weights: [0.0; 5],
            adjust: 0.0,
            direct: 0.0,
            continuation: window,
            after_speaking: window,
            restraint: Restraint {
                on: false,
                half_life: 0,
                cap: 0.0,
                k: 0.0,
            },
            severity_min: 0,
        },
        supersede_window: 0,
        judge: Judge {
            model: None,
            records: 0,
            max_tokens: 0,
            timeout: 0,
            moderation_timeout: 0,
            retries: 0,
            reason_chars: 0,
        },
        outbound: Outbound {
            quote_after: 0,
            mention_after: 0,
            min_bigrams: 0,
            similar: 0,
        },
        split_chars: 0,
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
