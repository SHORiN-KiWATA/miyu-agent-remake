//! 接模型那一步里填的那一屏（「第一次打开的引导」第 17、18 条）、试通了选模型那一屏的状态：有哪几格、试哪一家、筛模型。

use super::plan::{self, Target};
use super::rows;
use crate::oobe::field::Field;

/// 填的那一屏有哪几格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// 地址（自定义）。
    Url,
    /// 接口（自定义）：一格里写选着的那一个，左右换。
    Driver,
    /// 密钥。
    Key,
    /// 模型名（列不出模型时）。
    Model,
    /// 最后一行「测试连接 →」：`Enter` 试。
    Test,
}

/// 填的那一屏。
#[derive(Debug)]
pub struct Form {
    /// 自定义的是 `None`。
    pub provider: Option<rows::Provider>,
    /// 地址。
    pub url: Field,
    /// 接口：[`plan::DRIVERS`] 里第几种。
    pub driver: usize,
    /// 密钥。
    pub key: Field,
    /// 模型名。
    pub model: Field,
    /// 列不出模型：多一格模型名。
    pub needs_model: bool,
    /// 光标在第几格。
    pub focus: usize,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            provider: None,
            url: Field::url(),
            driver: 0,
            key: Field::secret(),
            model: Field::line(),
            needs_model: false,
            focus: 0,
        }
    }
}

impl Form {
    /// 有哪几格，照画的先后；有格子的最后一行是「测试连接」。
    pub fn slots(&self) -> Vec<Slot> {
        let mut out = match &self.provider {
            None => vec![Slot::Url, Slot::Driver, Slot::Key],
            Some(p) if p.ready() => Vec::new(),
            Some(_) => vec![Slot::Key],
        };
        if self.needs_model {
            out.push(Slot::Model);
        }
        if !out.is_empty() {
            out.push(Slot::Test);
        }
        out
    }

    /// 光标那一格正在编辑。
    pub fn editing(&self) -> bool {
        self.slot().is_some_and(|s| {
            matches!(s, Slot::Url | Slot::Key | Slot::Model)
                && self.field_ref(s).is_some_and(Field::editing)
        })
    }

    fn field_ref(&self, slot: Slot) -> Option<&Field> {
        match slot {
            Slot::Url => Some(&self.url),
            Slot::Key => Some(&self.key),
            Slot::Model => Some(&self.model),
            Slot::Driver | Slot::Test => None,
        }
    }

    /// 光标那一格。
    pub fn slot(&self) -> Option<Slot> {
        self.slots().get(self.focus).copied()
    }

    pub(super) fn field(&mut self, slot: Slot) -> Option<&mut Field> {
        match slot {
            Slot::Url => Some(&mut self.url),
            Slot::Key => Some(&mut self.key),
            Slot::Model => Some(&mut self.model),
            Slot::Driver | Slot::Test => None,
        }
    }

    /// 试哪一家；自定义的地址不对交回 `None`。
    pub(super) fn target(&self) -> Option<Target> {
        match &self.provider {
            Some(p) => Some(Target::Listed(p.clone())),
            None => plan::url(self.url.text()).map(|base_url| Target::Custom {
                driver: self.driver,
                base_url,
            }),
        }
    }
}

/// 试通了，选模型。
#[derive(Debug, Default)]
pub struct Models {
    /// 试的那一家、填的字。
    pub form: Form,
    /// 列出来的模型，试的那一个排第一。
    pub names: Vec<String>,
    /// 搜索。
    pub filter: Field,
    /// 选着对得上的第几个。
    pub cursor: usize,
}

impl Models {
    /// 照搜索的字对得上的，照原来的先后。
    pub fn matches(&self) -> Vec<&String> {
        let query = self.filter.text().to_lowercase();
        self.names
            .iter()
            .filter(|n| query.is_empty() || n.to_lowercase().contains(&query))
            .collect()
    }
}
