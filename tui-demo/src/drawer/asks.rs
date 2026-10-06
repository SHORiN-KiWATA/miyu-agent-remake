//! 核心推来的确认、提问记在哪（蓝图「确认和提问的抽屉」第 1、6 条）：问过的抽屉照调用编号留一份（交了被拒要重新打开、
//! 推来回答时写结果要用它的题目），了结过的调用记下（补发以前的事件时，先来了结、后来问的不开）。

use std::collections::{HashMap, HashSet};

use super::Drawer;

/// 问过的、了结过的。
#[derive(Debug, Default)]
pub struct Asks {
    known: HashMap<String, Drawer>,
    settled: HashSet<String>,
}

impl Asks {
    /// 来了一问：了结过的交回 `false`（不开），别的记下。
    pub fn asked(&mut self, drawer: &Drawer) -> bool {
        if self.settled.contains(&drawer.call_id) {
            return false;
        }
        self.known.insert(drawer.call_id.clone(), drawer.clone());
        true
    }

    /// 这一次调用了结了；交回问的时候那个抽屉（写结果用）。
    pub fn settle(&mut self, call_id: &str) -> Option<Drawer> {
        self.settled.insert(call_id.to_string());
        self.known.remove(call_id)
    }

    /// 交了被拒、要重新打开的那一个。
    pub fn again(&self, call_id: &str) -> Option<Drawer> {
        self.known.get(call_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::Asks;
    use crate::drawer::{Asked, Drawer};

    fn drawer(call: &str) -> Drawer {
        let asked: Asked = serde_json::from_value(serde_json::json!({
            "call_id": call, "questions": [{"question": "好吗？", "options": [{"label": "好"}]}]
        }))
        .unwrap();
        Drawer::question(None, asked)
    }

    #[test]
    fn a_settled_call_is_not_opened_again_and_a_refused_one_can_be() {
        let mut asks = Asks::default();
        assert!(asks.asked(&drawer("c1")));
        assert!(asks.again("c1").is_some(), "被拒了能重新打开");
        assert!(asks.settle("c1").is_some(), "写结果要用它的题目");
        assert!(!asks.asked(&drawer("c1")), "补发里先了结、后问的不开");
        assert!(asks.settle("c2").is_none());
    }
}
