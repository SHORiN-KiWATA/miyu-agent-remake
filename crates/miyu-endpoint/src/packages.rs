//! 软件包清单（施工 9-1 上，`docs/blueprint/packages.md`）：核心起来时读一次（[`load`]），`package.list` 照它答；装、卸、
//! 改了清单要重启核心才认。`miyu check` 照磁盘上现在的查（`check.rs`）。名字、说明照连接的语言挑，挑法同 `persona.list`；
//! 写错的、撞了的、协议版本对不上的带代码和给人看的一句。

use serde_json::{Value, json};

use std::collections::BTreeSet;

use miyu_config::package::{Code, Manifest, PackageKind, Problem, settings};
use miyu_config::{Item, Words};
use miyu_kernel::id::AccountId;
use miyu_policy::features::{Feature, Features};
use miyu_store::human::Human;
use miyu_store::packages::{Found, Issue, Packages, migrate};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::Core;

pub(crate) mod local;
pub(crate) mod manage;
pub(crate) mod methods;
pub(crate) mod status;
pub(crate) mod switch;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;
pub use status::absent;

/// 核心说的协议主版本。
const PROTOCOL: u32 = 1;

/// 运行日志的目标。
const TARGET: &str = "miyu::packages";

/// 这个核心的两层清单。
pub(crate) fn packages(core: &Core) -> Packages {
    Packages::new(&core.resources, &core.root, &core.admin)
}

/// 核心起来时读一次：写错的、撞了的各记一行 `WARN package invalid`，读不了的 `WARN package unreadable`。读之前把家目录里
/// 以前的写法挪成新的（施工 F-8 上），挪了的记一行 `INFO package moved`，挪不了的 `WARN package not moved`；写了 `kind` 的
/// 改成照表认的（施工 F-8 上补），`INFO package rewritten`、`WARN package not rewritten`。
pub fn load(resources: &ResourceRoot, root: &DataRoot, admin: &AccountId) -> Vec<Found> {
    let packages = Packages::new(resources, root, admin);
    if let Some(home) = packages.home_dir() {
        for moved in migrate::old_layout(home) {
            match &moved.result {
                Ok(()) => {
                    tracing::info!(target: TARGET, package = moved.id.as_str(), "package moved");
                }
                Err(why) => {
                    tracing::warn!(target: TARGET, package = moved.id.as_str(), why = ?why, "package not moved");
                }
            }
        }
        for rewritten in migrate::old_kind(home) {
            match &rewritten.result {
                Ok(()) => {
                    tracing::info!(target: TARGET, package = rewritten.id.as_str(), "package rewritten");
                }
                Err(why) => {
                    tracing::warn!(target: TARGET, package = rewritten.id.as_str(), why = ?why, "package not rewritten");
                }
            }
        }
    }
    let found = packages.read();
    // 本地库（施工 F-8 中上）：以前装的、还没记的补一份。
    local::backfill(&packages, &found);
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

/// `package.list`：核心这时认的（起来时读的，装卸以后当场换，施工 F-5 上），照编号排；卸掉的出厂的接在后面，带
/// `removed: true`（施工 F-5 上），好让头给人装回来。
pub(crate) fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let places = packages(core);
    let mut items: Vec<Value> = core
        .packages()
        .iter()
        .map(|found| listed(core, found, &places, &words, peer.language, false))
        .collect();
    items.extend(
        places
            .read_removed()
            .iter()
            .map(|found| listed(core, found, &places, &words, peer.language, true)),
    );
    Ok(json!({ "packages": items }))
}

/// 一项。`removed` 是卸掉了的出厂的包（带 `removed: true`）。
fn listed(
    core: &Core,
    found: &Found,
    places: &Packages,
    words: &Human,
    language: &str,
    removed: bool,
) -> Value {
    let mut item = json!({"package": found.id, "layer": found.layer.as_str()});
    if removed {
        item["removed"] = json!(true);
    }
    match &found.read {
        Ok(manifest) => {
            fill(&mut item, &found.id, manifest, language);
            // 施工 F-6 上（`package-pages.md`）：状态、开关、后台页。
            item["status"] = json!(status::status(core, &found.id, manifest, removed));
            let shipped = found.layer == miyu_store::packages::Layer::Shipped;
            if let Some(on) = status::enabled(core, &found.id, manifest, shipped, removed) {
                item["enabled"] = json!(on);
            }
            if status::page(found, manifest) {
                item["page"] = json!(true);
            }
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
    if let Some(protocol) = manifest.protocol {
        item["protocol"] = json!(protocol);
    }
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
    if let Some(icon) = &manifest.icon {
        item["icon"] = json!(icon);
    }
    if let Some(mascot) = &manifest.mascot {
        item["mascot"] = json!({"model": mascot.model});
    }
}

/// 说的协议版本不包含核心的：交回给人看的那个范围，例如 `2–3`；包含的、不说协议的（吉祥物包）没有。
pub(crate) fn mismatch(manifest: &Manifest) -> Option<String> {
    let [low, high] = manifest.protocol?;
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
    settle_with(found, core_items, &|manifest| !absent(manifest))
}

/// 同 [`settle`]，程序在不在照 `present` 认：生成配置的样本、测出厂的清单时照发行包的样子算，程序都在 `miyu` 旁边（施工
/// F-6 上：不照跑测试的这台机器）。
pub fn settle_with(
    found: &mut [Found],
    core_items: &[Item],
    present: &dyn Fn(&Manifest) -> bool,
) -> Vec<Item> {
    let modules: BTreeSet<&str> = core_items
        .iter()
        // 「软件包」那一页的是包的：`miyu check` 照起来以后的清单认，里面已经有包的项。
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
        // 程序不在的扩展、小程序当没装，配置项不进（施工 F-6 上，`package-pages.md`「程序不在就当没装」）。
        if !present(manifest) {
            continue;
        }
        // 一律在「软件包」那一页、这个包那一组（施工 F-6 上：「接入」页去掉了）。
        items.extend(settings::items(&one.id, &manifest.settings));
    }
    items
}

/// 清单是内置包、`built_in` 里没有的（施工 F-2，设计 30 第二节第 3 条）：这一份核心没编进它的代码，照读坏了的清单报
/// `not_built_in`、记一行 `WARN package invalid`。核心起来时读完清单调一次（`miyu-core`）。
pub fn compiled(found: &mut [Found], built_in: &[&str]) {
    for one in found.iter_mut() {
        let lacking = one.read.as_ref().is_ok_and(|manifest| {
            manifest.kind == PackageKind::Builtin && !built_in.contains(&one.id.as_str())
        });
        if !lacking {
            continue;
        }
        let problem = Problem {
            line: None,
            code: Code::NotBuiltIn,
            detail: one.id.clone(),
            message: format!(
                "this core does not have the code of built-in package {}",
                one.id
            ),
        };
        tracing::warn!(target: TARGET, package = one.id.as_str(), file = %one.path.display(), error = %problem, "package invalid");
        one.read = Err(Issue::Wrong(problem));
    }
}

/// 内置包 `id` 装了（施工 F-2）：有它读成了的清单，种类是内置。编进来的代码照它起不起。
pub fn is_installed(found: &[Found], id: &str) -> bool {
    found.iter().any(|one| {
        one.id == id
            && one
                .read
                .as_ref()
                .is_ok_and(|manifest| manifest.kind == PackageKind::Builtin)
    })
}

/// 装了的功能（施工 F-3 上，设计 30 第三节）：读成了的内置包、扩展包带的，照清单读的先后（包照编号，包里照写的先后）；
/// 没写功能的包整个算一个（[`Manifest::features_of`]）。界面、小程序不带。预设照它开关、工具照它归。
pub fn features(found: &[Found]) -> Features {
    Features::new(
        found
            .iter()
            .filter_map(|one| one.read.as_ref().ok().map(|manifest| (one, manifest)))
            // 程序不在的扩展当没装，它的功能不算（施工 F-6 上）。
            .filter(|(_, manifest)| !absent(manifest))
            .flat_map(|(one, manifest)| {
                manifest
                    .features_of(&one.id)
                    .into_iter()
                    .map(|feature| Feature {
                        id: feature.id,
                        package: one.id.clone(),
                        tools: feature.tools,
                    })
            })
            .collect(),
    )
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
