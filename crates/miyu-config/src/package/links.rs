//! 包和别处的几条连线（施工 F-1，设计 `30-插件框架.md` 第五节、第六节，`docs/blueprint/packages.md`「格式」）：平台接入
//! `[connection]`、依赖的小程序 `[depends]`、`[recommends]`、小程序自己的 `[worker]`。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, program_name};
use super::{Code, Problem};
use crate::secret::valid_name;

/// `[connection]`：这个扩展接的是哪个通讯平台。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// 平台名，写法同包的编号，例如 `qq`。
    pub platform: String,
}

/// `[worker]`：小程序怎么拉起。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worker {
    /// 程序名，不带路径；只找 `miyu` 旁边的（`packages.md`「转交」）。
    pub program: String,
    /// 拉起时带的参数；没写的是空的。
    pub args: Vec<String>,
}

/// `[connection]`：`platform` 必写。
pub(super) fn connection(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Connection, Problem> {
    reader.only(table, "connection", &["platform"])?;
    let item = reader.required(table, at, "connection", "platform")?;
    let platform = item.as_str().filter(|name| valid_name(name)).ok_or_else(|| {
        reader.problem(
            Some(item),
            Code::BadPlatform,
            "connection.platform",
            "connection.platform must start with a lowercase letter and use only lowercase letters, digits, - and _".to_string(),
        )
    })?;
    Ok(Connection {
        platform: platform.to_string(),
    })
}

/// `[depends]`、`[recommends]`（`name` 是哪一张）：`workers` 是小程序的包编号，不重复；没写的是空的。
pub(super) fn workers(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    name: &str,
) -> Result<Vec<String>, Problem> {
    reader.only(table, name, &["workers"])?;
    let workers = reader.texts(table, name, "workers")?;
    for (index, id) in workers.iter().enumerate() {
        let why = if !valid_name(id) {
            "is not a package id"
        } else if workers[..index].contains(id) {
            "is listed twice"
        } else {
            continue;
        };
        return Err(reader.problem(
            table.get("workers"),
            Code::BadDependency,
            id,
            format!("{name}.workers: {id:?} {why}"),
        ));
    }
    Ok(workers)
}

/// `[worker]`：`program` 必写。
pub(super) fn worker(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Worker, Problem> {
    reader.only(table, "worker", &["program", "args"])?;
    let item = reader.required(table, at, "worker", "program")?;
    let program = item
        .as_str()
        .filter(|program| program_name(program))
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadProgram,
                "worker.program",
                "worker.program must be a program name without a path".to_string(),
            )
        })?;
    Ok(Worker {
        program: program.to_string(),
        args: reader.texts(table, "worker", "args")?,
    })
}

#[cfg(test)]
mod tests;
