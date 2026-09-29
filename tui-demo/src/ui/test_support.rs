//! 正文排版测试用的夹具。

use std::cell::RefCell;

use miyu_store::human::Human;

use crate::config::Config;
use crate::core::Level;
use crate::figures::Figures;
use crate::ui::rows::{Ctx, MdCache};

/// 排版要的东西：照出厂的配置，没有工具显示名（显示名就是工具名本身）。
pub struct Fixture {
    pub config: Config,
    human: Human,
    md: RefCell<MdCache>,
    figures: RefCell<Figures>,
}

impl Fixture {
    pub fn new() -> Self {
        let config = Config::builtin().unwrap();
        let figures = RefCell::new(Figures::start(None, &config.figures, None, |_| true));
        Self {
            config,
            human: Human::default(),
            md: RefCell::new(MdCache::new()),
            figures,
        }
    }

    pub fn ctx(&self) -> Ctx<'_> {
        Ctx {
            config: &self.config,
            human: &self.human,
            indent: String::new(),
            width: 60,
            hover: None,
            frame: 0,
            md: &self.md,
            figures: &self.figures,
            level: Level::Workspace,
        }
    }
}
