//! 核心推来的确认、提问记在哪（蓝图「确认和提问的抽屉」第 1、6 条）：问过的抽屉照调用编号留一份（交了被拒要重新打开、
//! 推来回答时写结果要用它的题目），了结过的调用记下（补发以前的事件时，先来了结、后来问的不开）。调用编号只在
//! 一个会话里唯一，一律连问的会话一起认。

use std::collections::{HashMap, HashSet};

use super::Drawer;

/// 哪个会话（整个编号）的哪次调用。
type Key = (String, String);

/// 问过的、了结过的。
#[derive(Debug, Default)]
pub struct Asks {
    known: HashMap<Key, Drawer>,
    settled: HashSet<Key>,
}

fn key(owner: &str, call_id: &str) -> Key {
    (owner.to_string(), call_id.to_string())
}

impl Asks {
    /// 来了一问：了结过的交回 `false`（不开），别的记下。
    pub fn asked(&mut self, drawer: &Drawer) -> bool {
        let key = key(&drawer.owner, &drawer.call_id);
        if self.settled.contains(&key) {
            return false;
        }
        self.known.insert(key, drawer.clone());
        true
    }

    /// 这一次调用了结了；交回问的时候那个抽屉（写结果用）。
    pub fn settle(&mut self, owner: &str, call_id: &str) -> Option<Drawer> {
        let key = key(owner, call_id);
        self.settled.insert(key.clone());
        self.known.remove(&key)
    }

    /// 交了被拒、要重新打开的那一个。
    pub fn again(&self, owner: &str, call_id: &str) -> Option<Drawer> {
        self.known.get(&key(owner, call_id)).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::Asks;
    use crate::drawer::{Asked, Drawer};

    fn drawer(owner: &str, call: &str) -> Drawer {
        let asked: Asked = serde_json::from_value(serde_json::json!({
            "call_id": call, "questions": [{"question": "好吗？", "options": [{"label": "好"}]}]
        }))
        .unwrap();
        let mut drawer = Drawer::question(None, asked);
        drawer.owner = owner.into();
        drawer
    }

    #[test]
    fn a_settled_call_is_not_opened_again_and_a_refused_one_can_be() {
        let mut asks = Asks::default();
        assert!(asks.asked(&drawer("s1", "c1")));
        assert!(asks.again("s1", "c1").is_some(), "被拒了能重新打开");
        assert!(asks.settle("s1", "c1").is_some(), "写结果要用它的题目");
        assert!(!asks.asked(&drawer("s1", "c1")), "补发里先了结、后问的不开");
        assert!(asks.settle("s1", "c2").is_none());
        assert!(asks.asked(&drawer("s2", "c1")), "别的会话同一个编号照开");
    }
}
