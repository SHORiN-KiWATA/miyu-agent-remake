//! 参数的表（`docs/blueprint/chat.md` 第一条「参数的表」、第八条「怎么走」第 2、4 条，施工 O-15）：场所规则里的
//! `chatty = { … }` 和出厂文件里的 `[chatty]` 读法一样，一份。展开成一项一项（`表.项`），每一项照第八条的声明
//! （`params/items.rs`）查；表写成别的只丢那一张表，项写错的只丢那一项。问题照第一条的规矩报，位置、原文取法一样。
//!
//! 出厂文件另有两条（[`defaults`]）：最上面只认声明里的表；每一项都得写，缺了的报 `wrong_type`（配置的原因码里没有
//! 「缺了」，不改配置，第八条施工时定的第 9 条）。

use miyu_config::Value;
use miyu_config::problem::{Code, nearest};
use toml_edit::{Item as Node, Key};

use crate::params::items::{self, Item};

use super::{File, Problem, Reader};

/// 读好的一项：声明里的哪一项、查过的值、键所在的行（第一条施工时定的第 10 条：来处的行号是键所在的那一行）。
pub(super) type Found = (&'static Item, Value, usize);

impl Reader<'_> {
    /// 一张参数表：`table` 是声明里的表名，`key`、`node` 是写的那一格；`rule` 是第几条规则，出厂文件没有。
    ///
    /// 不是表的（`chatty = 5`、`[[rule.chatty]]`）`wrong_type`，整张表不收。表里每一项照声明读：不认识的 `unknown_key`，
    /// 给这张表里离得最近的项，写成 `表.项`；写错的照原因码报，只丢那一项。交回写对的几项，照写的先后。
    pub(super) fn table(
        &mut self,
        rule: Option<usize>,
        table: &str,
        key: &Key,
        node: &Node,
    ) -> Vec<Found> {
        let Some(items) = node.as_table_like() else {
            let problem = self.item(Code::WrongType, rule, table, key, node, false);
            self.problems.push(problem);
            return Vec::new();
        };
        let mut found = Vec::new();
        for (name, _) in items.iter() {
            let Some((item_key, item_node)) = items.get_key_value(name) else {
                continue;
            };
            let full = format!("{table}.{name}");
            let Some(item) = items::in_table(table, name) else {
                let mut problem =
                    self.item(Code::UnknownKey, rule, &full, item_key, item_node, true);
                problem.suggest =
                    nearest(items::names(table), name).map(|near| format!("{table}.{near}"));
                self.problems.push(problem);
                continue;
            };
            let read = item_node
                .as_value()
                .ok_or(Code::WrongType)
                .and_then(|value| item.read(value));
            match read {
                Ok(value) => found.push((item, value, self.at(item_key.span()).line)),
                Err(code) => {
                    let problem = self.item(code, rule, &full, item_key, item_node, false);
                    self.problems.push(problem);
                }
            }
        }
        found
    }
}

/// 读出厂参数的文件（第八条「怎么走」第 2 条）：最上面只认声明里的表，表里每一项照 [`Reader::table`] 读；读完照行、列排好
/// 问题，再照声明的先后查缺了的。写了但写错的、整张表写成别的，已经报过，不再报缺（第八条「怎么走」第 2 条）。
///
/// # Errors
///
/// 有一条问题（警告也算）就整份不用，交回全部问题：出厂文件是打包的，有问题是打包的错（第八条施工时定的第 10 条）。
pub(crate) fn defaults(file: &File) -> Result<Vec<(&'static Item, Value)>, Vec<Problem>> {
    let mut reader = Reader::new(file);
    let Some(document) = reader.parse() else {
        return Err(reader.problems);
    };
    let root = document.as_table();
    let mut found = Vec::new();
    for (name, _) in root.iter() {
        let Some((key, node)) = root.get_key_value(name) else {
            continue;
        };
        match items::table(name) {
            Some(table) => found.extend(reader.table(None, table, key, node)),
            None => {
                let mut problem = reader.item(Code::UnknownKey, None, name, key, node, true);
                problem.suggest = nearest(items::tables(), name).map(String::from);
                reader.problems.push(problem);
            }
        }
    }
    reader.sort();
    let reported = |problems: &[Problem], key: &str| {
        problems
            .iter()
            .any(|problem| problem.key.as_deref() == Some(key))
    };
    for item in items::ITEMS.iter().filter(|item| !item.optional) {
        let written = found.iter().any(|(found, ..)| found.key == item.key);
        if !written
            && !reported(&reader.problems, item.key)
            && !reported(&reader.problems, item.table())
        {
            let missing = reader.problem(Code::WrongType, None, Some(item.key.to_string()));
            reader.problems.push(missing);
        }
    }
    match reader.problems.is_empty() {
        true => Ok(found
            .into_iter()
            .map(|(item, value, _)| (item, value))
            .collect()),
        false => Err(reader.problems),
    }
}

#[cfg(test)]
mod tests;
