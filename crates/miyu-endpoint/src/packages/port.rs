//! 看软件包的端口（施工 F-10 上，`miyu_tool::PackagesPort`）：`packages` 这件工具经它看。照协议那几样回应算（`package.list`、
//! `package.info`、`package.install` 带 `preview` 的），名字、说明、写错的那一句照英文：她看的字是英文。拿着核心的弱引用：
//! 会话活在核心里面，拿强的会绕成环。

use std::path::PathBuf;
use std::sync::{Arc, Weak};

use serde_json::{Value, json};

use miyu_tool::{Looking, PackageRefusal, PackagesPort};

use super::local::InfoParams;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::{Locale, Refusal};

/// 交给会话的那一份。
pub(crate) fn port(core: &Arc<Core>) -> Arc<dyn PackagesPort> {
    Arc::new(Port(Arc::downgrade(core)))
}

/// 核心那一头。
struct Port(Weak<Core>);

impl Port {
    /// 核心还在；正在停的拒 `shutting_down`。
    fn core(&self) -> Result<Arc<Core>, PackageRefusal> {
        self.0.upgrade().ok_or_else(|| PackageRefusal {
            reason: "shutting_down".to_string(),
            problem: None,
            line: None,
        })
    }
}

/// 照英文说：名字、说明、清单哪里不对。
fn english() -> Peer {
    Peer {
        locale: Locale::En,
        language: "en",
        input: false,
    }
}

/// 协议上的拒绝换成端口的：原因代码，写错的清单带上哪里不对、第几行。
fn refused(refusal: Refusal) -> PackageRefusal {
    let data = refusal.data.unwrap_or_default();
    PackageRefusal {
        reason: refusal.reason.to_string(),
        problem: data
            .get("problem")
            .and_then(Value::as_str)
            .map(str::to_string),
        line: data.get("line").and_then(Value::as_u64),
    }
}

impl PackagesPort for Port {
    fn list(&self) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let listed = super::list(&core, english()).map_err(refused)?;
            Ok(listed["packages"].clone())
        })
    }

    fn info(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let params: InfoParams = serde_json::from_value(json!({ "package": package }))
                .map_err(|_| refused(Refusal::BAD_PARAMS))?;
            let mut info = super::local::info(&core, params).await.map_err(refused)?;
            if let Some(entry) = super::entry(&core, &package, english()) {
                for key in ["name", "summary", "kind"] {
                    if let Some(value) = entry.get(key) {
                        info[key] = value.clone();
                    }
                }
            }
            Ok(info)
        })
    }

    fn inspect(&self, path: PathBuf) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            super::preview::installing(&core, english(), &path)
                .await
                .map_err(refused)
        })
    }
}
