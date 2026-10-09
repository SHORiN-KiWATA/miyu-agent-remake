//! 工具目录（05 第八节，施工 4-1）：核心起来时登记一次；提供者登记一次换出新的一份（施工 O-2 上，放在 [`crate::Shelf`]
//! 上）。照名字排好，造会话时照这个先后交出工具面。改过名的工具照以前的名字也找得到（施工 7-5 再补，[`Tool::formerly`]）。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::{Spec, Tool};

/// 工具名最长多少个字符：各家供应商对函数名的限制（05 第六节）。
const NAME_LIMIT: usize = 64;

/// 工具目录。
#[derive(Clone, Default)]
pub struct Catalog {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    /// 以前的名字到现在的名字（施工 7-5 再补）。
    formerly: BTreeMap<String, String>,
    /// 每件工具现在的名字到它的软件包（施工 P-2 中）：预设照包开关。
    packages: BTreeMap<String, String>,
    /// 经提供者登记过的包（施工 O-2 中，[`Catalog::replacing`]）：回合开头换快照时，只有它们的工具照现在的登记换。
    provided: BTreeSet<String>,
}

/// 基础系统的编号（`10-自带软件.md` 第三节）：只交一串工具的老写法，全算它。
pub const BASESYSTEM: &str = "basesystem";

/// 一件工具登记不上：是哪一件，哪一条没过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    /// 那件工具的名字。
    pub tool: String,
    /// 哪一条没过。
    pub problem: Problem,
}

/// 登记时查的几条（05 第六节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// 已经有一件同名的：她调的是哪一件，说不清。以前的名字也算（施工 7-5 再补）：撞上别的工具现在的、以前的名字，
    /// 同样说不清。
    Duplicate,
    /// 名字不合写法：只用英文字母、数字、`_`、`-`，1 到 64 个字符。不合的，每次请求都会被供应商拒收。
    Name,
    /// 参数格式不是 `{"type":"object",…}`：供应商只收对象。
    Parameters,
}

impl Catalog {
    /// 登记这几件，照名字排好。有一件查不过，整个目录都登记不上，报交进来时排在前面的那一件。以前的名字跟着它现在的名字
    /// 登记，只查重名：撞上的报那个以前的名字。
    ///
    /// # Errors
    ///
    /// 有两件同名的（以前的名字也算）、名字不合写法的、参数格式不是对象的。
    pub fn new(tools: impl IntoIterator<Item = Arc<dyn Tool>>) -> Result<Catalog, CatalogError> {
        Catalog::in_packages([(BASESYSTEM, tools.into_iter().collect::<Vec<_>>())])
    }

    /// 照软件包登记（施工 P-2 中）：每一组是一个包的编号和它的几件，记下每件归哪个包；查法同 [`Catalog::new`]，两个包里
    /// 同名的也算重名。
    ///
    /// # Errors
    ///
    /// 同 [`Catalog::new`]。
    pub fn in_packages<'a>(
        groups: impl IntoIterator<Item = (&'a str, Vec<Arc<dyn Tool>>)>,
    ) -> Result<Catalog, CatalogError> {
        let mut catalog = Catalog::default();
        for (package, tools) in groups {
            for tool in tools {
                catalog.add(package, tool)?;
            }
        }
        Ok(catalog)
    }

    /// 换掉包 `package` 的工具（施工 O-2 上，提供者再登记一次）：交回新的一份，别的包的照留，原来这份不动；没有这个包的是
    /// 加进来。查法同 [`Catalog::new`]，撞上别的包的也算重名。
    ///
    /// # Errors
    ///
    /// 同 [`Catalog::new`]。
    pub fn replacing(
        &self,
        package: &str,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Result<Catalog, CatalogError> {
        let mut catalog = self.clone();
        catalog.take_out(package);
        catalog.provided.insert(package.to_string());
        for tool in tools {
            catalog.add(package, tool)?;
        }
        Ok(catalog)
    }

    /// 拿掉包 `package` 的工具和它们以前的名字。
    fn take_out(&mut self, package: &str) {
        let names: Vec<String> = self
            .packages
            .iter()
            .filter(|(_, owner)| *owner == package)
            .map(|(name, _)| name.clone())
            .collect();
        for name in &names {
            self.tools.remove(name);
            self.packages.remove(name);
        }
        self.formerly.retain(|_, now| !names.contains(now));
    }

    /// 登记一件：查过了放进目录，记下它的包。
    fn add(&mut self, package: &str, tool: Arc<dyn Tool>) -> Result<(), CatalogError> {
        let spec = tool.spec();
        let problem = if !name_is_valid(&spec.name) {
            Some(Problem::Name)
        } else if !takes_an_object(spec) {
            Some(Problem::Parameters)
        } else if self.taken(&spec.name) {
            Some(Problem::Duplicate)
        } else {
            None
        };
        if let Some(problem) = problem {
            return Err(CatalogError {
                tool: spec.name.clone(),
                problem,
            });
        }
        for former in tool.formerly() {
            if self.taken(former) {
                return Err(CatalogError {
                    tool: (*former).to_string(),
                    problem: Problem::Duplicate,
                });
            }
            self.formerly
                .insert((*former).to_string(), spec.name.clone());
        }
        self.packages.insert(spec.name.clone(), package.to_string());
        self.tools.insert(spec.name.clone(), tool);
        Ok(())
    }

    /// 叫 `name` 的那一件归哪个包（以前的名字也算）；没有这件的没有。
    pub fn package_of(&self, name: &str) -> Option<&str> {
        let now = self.formerly.get(name).map_or(name, String::as_str);
        self.packages.get(now).map(String::as_str)
    }

    /// 叫 `name` 的那一件是经提供者登记的（施工 O-2 中）；没有这件的不是。
    pub fn provided(&self, name: &str) -> bool {
        self.package_of(name)
            .is_some_and(|package| self.provided.contains(package))
    }

    /// 有工具的几个包，照编号排、不重复。
    pub fn packages(&self) -> impl Iterator<Item = &str> {
        self.packages
            .values()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    /// `name` 已经有主了：是一件工具现在的名字，或者以前的名字。
    fn taken(&self, name: &str) -> bool {
        self.tools.contains_key(name) || self.formerly.contains_key(name)
    }

    /// 每件的规格，照名字的先后。
    pub fn specs(&self) -> impl Iterator<Item = &Spec> {
        self.tools.values().map(|tool| tool.spec())
    }

    /// 叫 `name` 的那一件（施工 4-2）；以前叫 `name` 的也算（施工 7-5 再补）：改名以前造的会话，快照里冻着旧名字。
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        let now = self.formerly.get(name).map_or(name, String::as_str);
        self.tools.get(now)
    }
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.tools.keys()).finish()
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let problem = match self.problem {
            Problem::Duplicate => "another tool has the same name",
            Problem::Name => "the name must be 1 to 64 ASCII letters, digits, '_' or '-'",
            Problem::Parameters => "the parameters must be a JSON Schema of type \"object\"",
        };
        write!(f, "tool {:?}: {problem}", self.tool)
    }
}

impl std::error::Error for CatalogError {}

/// 名字合不合写法：英文字母、数字、`_`、`-`，1 到 64 个字符。
fn name_is_valid(name: &str) -> bool {
    (1..=NAME_LIMIT).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// 参数格式是不是 `{"type":"object",…}`。
fn takes_an_object(spec: &Spec) -> bool {
    serde_json::from_str::<Value>(spec.parameters.get())
        .ok()
        .as_ref()
        .and_then(|schema| schema.get("type"))
        .and_then(Value::as_str)
        == Some("object")
}

#[cfg(test)]
mod tests;
