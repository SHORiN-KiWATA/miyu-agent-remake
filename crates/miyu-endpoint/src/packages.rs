//! 软件包清单（施工 9-1 上，`docs/blueprint/packages.md`）：核心起来时读一次（[`load`]），`package.list` 照它答；装、卸、
//! 改了清单要重启核心才认。`miyu check` 照磁盘上现在的查（`check.rs`）。名字、说明照连接的语言挑，挑法同 `persona.list`；
//! 写错的、撞了的、协议版本对不上的带代码和给人看的一句。

use serde_json::{Value, json};

use std::collections::BTreeSet;

use miyu_config::package::{Code, Manifest, PackageKind, Problem, settings};
use miyu_config::{Item, Words};
use miyu_kernel::id::AccountId;
use miyu_store::human::Human;
use miyu_store::packages::{Found, Issue, Packages};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// 核心说的协议主版本。
const PROTOCOL: u32 = 1;

/// 运行日志的目标。
const TARGET: &str = "miyu::packages";

/// 这个核心的两层清单。
pub(crate) fn packages(core: &Core) -> Packages {
    Packages::new(&core.resources, &core.root, &core.admin)
}

/// 核心起来时读一次：写错的、撞了的各记一行 `WARN package invalid`，读不了的 `WARN package unreadable`。
pub fn load(resources: &ResourceRoot, root: &DataRoot, admin: &AccountId) -> Vec<Found> {
    let found = Packages::new(resources, root, admin).read();
    for one in &found {
        match &one.read {
            Ok(_) => {}
            Err(Issue::Wrong(problem)) => {
                tracing::warn!(target: TARGET, package = one.id.as_str(), file = %one.path.display(), error = %problem, "package invalid");
            }
            Err(Issue::Unreadable(error)) => {
                tracing::warn!(target: TARGET, package = one.id.as_str(), file = %one.path.display(), error = %error, "package unreadable");
            }
        }
    }
    found
}

/// `package.list`：起来时读到的，照编号排。
pub(crate) fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let places = packages(core);
    let listed: Vec<Value> = core
        .packages
        .iter()
        .map(|found| listed(found, &places, &words, peer.language))
        .collect();
    Ok(json!({ "packages": listed }))
}

/// 一项。
fn listed(found: &Found, places: &Packages, words: &Human, language: &str) -> Value {
    let mut item = json!({"package": found.id, "layer": found.layer.as_str()});
    match &found.read {
        Ok(manifest) => {
            fill(&mut item, &found.id, manifest, language);
            item["state"] = json!(places.state_dir(&found.id).to_string_lossy());
            if let Some(range) = mismatch(manifest) {
                item["code"] = json!("protocol_mismatch");
                item["problem"] = json!(sentence(words, "protocol_mismatch", &range));
            }
        }
        Err(Issue::Wrong(problem)) => {
            item["code"] = json!(problem.code.as_str());
            item["problem"] = json!(
                sentence(words, problem.code.as_str(), &problem.detail)
                    .unwrap_or_else(|| problem.message.clone())
            );
            if let Some(line) = problem.line {
                item["line"] = json!(line);
            }
        }
        Err(Issue::Unreadable(error)) => {
            item["code"] = json!("unreadable");
            item["problem"] = json!(
                Words::sentence(words, "config/unreadable", &[("why", &error.to_string())])
                    .unwrap_or_else(|| error.to_string())
            );
        }
    }
    item
}

/// 读成了的几格；没有的不写。
fn fill(item: &mut Value, id: &str, manifest: &Manifest, language: &str) {
    item["kind"] = json!(manifest.kind.as_str());
    item["protocol"] = json!(manifest.protocol);
    item["name"] = json!(pick(&manifest.name, language));
    if let Some(version) = &manifest.version {
        item["version"] = json!(version);
    }
    if let Some(summary) = pick(&manifest.summary, language) {
        item["summary"] = json!(summary);
    }
    if let Some(command) = &manifest.command {
        item["command"] = json!({
            "name": command.name,
            "program": command.program,
            "about": pick(&command.about, language),
        });
    }
    if let Some(process) = &manifest.process {
        item["process"] = json!({"args": process.args, "start": process.start.as_str()});
    }
    if let Some(ui) = &manifest.ui {
        item["opens"] = json!(ui.opens);
        if let Some(dir) = &ui.pages_dir {
            item["pages_dir"] = json!(dir);
        }
    }
    if let Some(check) = &manifest.check {
        item["check"] = json!({"args": check.args});
    }
    links(item, id, manifest, language);
}

/// 施工 F-1 加的几格（设计 30）：必需的、带的功能（照包算的那一个也列，名字、说明照语言挑；不列工具）、平台接入、依赖、小程序。
fn links(item: &mut Value, id: &str, manifest: &Manifest, language: &str) {
    if manifest.required {
        item["required"] = json!(true);
    }
    if matches!(manifest.kind, PackageKind::Builtin | PackageKind::Process) {
        let features: Vec<Value> = manifest
            .features_of(id)
            .iter()
            .map(|feature| {
                let mut listed = json!({"id": feature.id, "name": pick(&feature.name, language)});
                if let Some(summary) = pick(&feature.summary, language) {
                    listed["summary"] = json!(summary);
                }
                listed
            })
            .collect();
        item["features"] = json!(features);
    }
    if let Some(connection) = &manifest.connection {
        item["connection"] = json!({"platform": connection.platform});
    }
    for (key, workers) in [
        ("depends", &manifest.depends),
        ("recommends", &manifest.recommends),
    ] {
        if !workers.is_empty() {
            item[key] = json!({"workers": workers});
        }
    }
    if let Some(worker) = &manifest.worker {
        item["worker"] = json!({"program": worker.program, "args": worker.args});
    }
}

/// 说的协议版本不包含核心的：交回给人看的那个范围，例如 `2–3`；包含的没有。
pub(crate) fn mismatch(manifest: &Manifest) -> Option<String> {
    let [low, high] = manifest.protocol;
    if (low..=high).contains(&PROTOCOL) {
        return None;
    }
    Some(if low == high {
        low.to_string()
    } else {
        format!("{low}–{high}")
    })
}

/// 包的配置项（施工 9-1 下，`packages.md`「配置项」）：读成了的清单的 `[settings]` 拼成配置项，键是 `<包>.<名字>`。包的编号
/// 是核心自己的某个模块（`core_items` 里键的第一段，「软件包」那一页的不算）的，那一份改报 `settings_taken`、一项都不收。
/// 核心起来时调一次：拼出来的项一直用到退出（`miyu_config::package::settings::items`）。
pub fn settle(found: &mut [Found], core_items: &[Item]) -> Vec<Item> {
    let modules: BTreeSet<&str> = core_items
        .iter()
        .filter(|item| item.ui.page != settings::PAGE)
        .filter_map(|item| item.key.split('.').next())
        .collect();
    let mut items = Vec::new();
    for one in found.iter_mut() {
        let Ok(manifest) = &one.read else {
            continue;
        };
        if manifest.settings.is_empty() {
            continue;
        }
        if modules.contains(one.id.as_str()) {
            one.read = Err(Issue::Wrong(Problem {
                line: None,
                code: Code::SettingsTaken,
                detail: one.id.clone(),
                message: format!(
                    "package {} has the same name as a core module and cannot declare settings",
                    one.id
                ),
            }));
            continue;
        }
        items.extend(settings::items(&one.id, &manifest.settings));
    }
    items
}

/// 读成了的清单：编号和样子（给人看的字照它并进包的配置项的名字）。
pub(crate) fn manifests(found: &[Found]) -> impl Iterator<Item = (&str, &Manifest)> {
    found.iter().filter_map(|one| match &one.read {
        Ok(manifest) => Some((one.id.as_str(), manifest)),
        Err(_) => None,
    })
}

/// 给人看的那一句：`package-problems/<code>`，`detail` 换进去。
pub(crate) fn sentence(words: &Human, code: &str, detail: &str) -> Option<String> {
    Words::sentence(
        words,
        &format!("package-problems/{code}"),
        &[("detail", detail)],
    )
}
