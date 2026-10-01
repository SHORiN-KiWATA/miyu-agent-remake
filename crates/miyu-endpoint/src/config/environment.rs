//! 核心的环境变量（`docs/blueprint/config.md`「怎么走」第二条第 5 条、第九条第 5 条）：带 `env` 的项照它压过配置，
//! `{ env = "<变量>" }` 照它取 key（施工 8-5）。
//!
//! 核心是拉起它的那个头的环境（`ipc.md`），起来以后不改自己的环境：进程里现在的就是起来时的，之后在别的终端里设的看不到，
//! 要等核心重启。测试换成手写的几个。值可能就是 key：`Debug` 不印任何一个。

use std::fmt;
use std::sync::Arc;

/// 照名字查一个环境变量的那个函数。
type Lookup = dyn Fn(&str) -> Option<String> + Send + Sync;

/// 照名字查一个环境变量。
#[derive(Clone)]
pub struct Environment(Arc<Lookup>);

impl fmt::Debug for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Environment(…)")
    }
}

impl Environment {
    /// 这个进程的环境。
    pub fn process() -> Environment {
        Environment(Arc::new(|name| std::env::var(name).ok()))
    }

    /// 手写的几个：名字、值（测试用、核心不给环境的时候）。
    pub fn of(pairs: &[(&str, &str)]) -> Environment {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        Environment(Arc::new(move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }))
    }

    /// 变量 `name` 的值；没设的是空的。
    pub fn get(&self, name: &str) -> Option<String> {
        (self.0)(name)
    }

    /// 变量 `name` 设了、去掉前后空白不是空的：`{ env }` 取得到 key（空的当没设，`env_not_set`）。
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some_and(|value| !value.trim().is_empty())
    }
}
