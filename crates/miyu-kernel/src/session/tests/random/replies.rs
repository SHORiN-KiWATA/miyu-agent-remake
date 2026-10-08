//! 随机测试里执行器替身像样的回报（从 `random.rs` 挪出来，施工 4-7 再补：那边放不下了）：在路上的那次请求往下说的一段增量、
//! 多半对得上的请求和调用、乱来的增量、挂接点交回来的注入。

use super::*;

impl Watch {
    /// 下一段增量：多半接着在路上的那次请求像样地往下说，偶尔乱来。工具调用的参数是一个
    /// 空对象，一次写完。新的一块三回里两回是工具调用；手动压缩那一轮只有摘要请求，倒过来，三回里一回（施工 6-8：
    /// 摘要回复里调了工具的取不到摘要，不然难得压成）。
    pub(super) fn some_delta(&mut self, rng: &mut Rng) -> Delta {
        if rng.below(if self.calm { 40 } else { 6 }) == 0 {
            return scrambled_delta(rng);
        }
        match self.open_block {
            None => {
                let index = self.next_block;
                self.next_block += 1;
                let roll = rng.below(3);
                let tool = match self.manual_turn() {
                    Some(_) => roll == 0,
                    None => roll > 0,
                };
                self.open_block = Some((index, tool, false));
                let kind = if tool {
                    let name = match self.writing {
                        true => ["read", "write", "write"][rng.below(3) as usize],
                        false => ["read", "read", "read", "write", "write", "reed"]
                            [rng.below(6) as usize],
                    };
                    Kind::ToolCall {
                        name: name.to_string(),
                    }
                } else {
                    Kind::Text
                };
                Delta::Start { index, kind }
            }
            Some((index, true, false)) => {
                self.open_block = Some((index, true, true));
                Delta::Text {
                    index,
                    text: "{}".to_string(),
                }
            }
            Some((index, true, true)) => {
                self.open_block = None;
                Delta::End { index }
            }
            Some((index, false, _)) if rng.below(3) == 0 => {
                self.open_block = None;
                Delta::End { index }
            }
            Some((index, false, _)) => Delta::Text {
                index,
                text: "x".to_string(),
            },
        }
    }

    /// 多半是在路上的那次请求，偶尔是对不上的。
    pub(super) fn some_seen(&self, rng: &mut Rng) -> Seq {
        match self.asking {
            Some(seen) if rng.below(5) > 0 => seen,
            _ => seq(1 + rng.below(self.last())),
        }
    }

    /// 多半是在跑的一个调用，偶尔是对不上的。
    pub(super) fn some_call(&self, rng: &mut Rng) -> CallId {
        let running: Vec<CallId> = self.running.iter().copied().collect();
        match running.len() {
            0 => CallId::new(seq(1 + rng.below(self.last())), 1).unwrap(),
            n if rng.below(5) > 0 => running[rng.below(n as u64) as usize],
            _ => CallId::new(seq(1 + rng.below(self.last())), 1 + rng.below(3) as u32).unwrap(),
        }
    }
}

/// 挂接点交回来的 0 到 2 块注入。
pub(super) fn some_injections(rng: &mut Rng) -> Vec<Injection> {
    (0..rng.below(3))
        .map(|k| Injection {
            module: ModuleId::parse(&format!("m{k}")).unwrap(),
            fact: ContextInjected {
                // 第二块带编号：内核原样记、原样交回挂接点（施工 R-4 上），看守照日志对。照第几块定、不再取随机数，别的
                // 输入的那一串随机数不变（走到的路照旧走得到）。
                refs: (0..k).map(|r| format!("r{r}")).collect(),
                kind: FactKind::parse("memory").unwrap(),
                text: format!("<memory n=\"{k}\"/>"),
            },
        })
        .collect()
}

/// 乱来的一段增量：头两块里的一块，正文或者工具调用；先后乱了的，累积器会报错。
fn scrambled_delta(rng: &mut Rng) -> Delta {
    let index = rng.below(2) as usize;
    match rng.below(4) {
        0 => Delta::Start {
            index,
            kind: if rng.below(3) == 0 {
                Kind::ToolCall {
                    name: "read".to_string(),
                }
            } else {
                Kind::Text
            },
        },
        1 | 2 => Delta::Text {
            index,
            text: "x".to_string(),
        },
        _ => Delta::End { index },
    }
}
