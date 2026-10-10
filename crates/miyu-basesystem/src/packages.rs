//! `packages`（`docs/blueprint/tools/packages.md`，施工 F-10 上，设计 `31-软件包.md` 第六节）：她看软件包。不带参数列出装了的
//! （一个一行：编号、名字、一句说明，关着的、卸掉的、读不了的另说），`package` 看一个，`path` 看一个包文件夹装上会是什么样。
//! 访问类别是读，哪一级都不问人；看包文件夹的照读那个路径判（只碰得到自己工作区的会话看不了外面的，数据根哪一级都不许）。
//! 经 [`Call::packages`] 交给核心，照协议那几样回应算；看一个、看文件夹的照原样交回那一份 JSON（英文，紧凑），不另编字：
//! 调用之后才用得上的知识都在里面。

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, PACKAGES, PackageRefusal, Progress, Running, Spec, Target, Tool};

use crate::common::said;
use crate::load::{self, LoadError, say};

/// `packages`。
pub(crate) struct Packages {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/packages/*.txt`。
#[derive(Clone)]
struct Texts {
    listed: Template,
    off: Template,
    removed: Template,
    broken: Template,
    inspected: Template,
    invalid: Template,
    invalid_line: Template,
    unknown: Template,
    refused: Template,
    both: Template,
    relative: Template,
    unavailable: Template,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

impl Packages {
    /// 照资源目录 `resources` 里的字造。访问类别是读：只看，什么都不改。
    pub(crate) fn load(resources: &Path) -> Result<Packages, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, PACKAGES, name, fields);
        let row = ["package", "name", "summary"];
        Ok(Packages {
            spec: load::spec(resources, PACKAGES, Access::Read)?,
            texts: Texts {
                listed: text("listed", &row)?,
                off: text("listed-off", &row)?,
                removed: text("listed-removed", &["package", "name"])?,
                broken: text("listed-broken", &["package", "problem"])?,
                inspected: text("inspected", &["path"])?,
                invalid: text("invalid", &["path", "problem"])?,
                invalid_line: text("invalid-line", &["path", "line", "problem"])?,
                unknown: text("unknown", &["package"])?,
                refused: text("refused", &["what", "reason"])?,
                both: text("both", &[])?,
                relative: text("relative", &[])?,
                unavailable: text("unavailable", &[])?,
            },
        })
    }
}

impl Tool for Packages {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    /// 看包文件夹的报那个路径是读（施工 F-10 上）：权限策略照它判，只碰得到自己工作区的会话看不了工作区外的文件夹，数据根
    /// 哪一级都不许。列出、看一个不碰路径。
    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .ok()
            .and_then(|args| args.path)
            .map(|path| {
                vec![Target {
                    path,
                    write: false,
                    itself: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args: Args = serde_json::from_str(&call.args).unwrap_or(Args {
                package: None,
                path: None,
            });
            let Some(port) = call.packages.clone() else {
                return Done::error(say(&texts.unavailable, &[])).said(said("packages/failed"));
            };
            let done = match (args.package, args.path) {
                (Some(_), Some(_)) => Err(say(&texts.both, &[])),
                (None, None) => port
                    .list()
                    .await
                    .map(|listed| (texts.rows(&listed), said("packages/listed")))
                    .map_err(|refusal| texts.refused(&refusal, "packages")),
                (Some(package), None) => match port.info(package.clone()).await {
                    Ok(info) => Ok((
                        format!("{info}\n"),
                        said("packages/shown").with("package", &package),
                    )),
                    Err(refusal) if refusal.reason == "unknown_package" => {
                        Err(say(&texts.unknown, &[("package", &package)]))
                    }
                    Err(refusal) => Err(texts.refused(&refusal, &package)),
                },
                (None, Some(path)) => texts.inspect(&*port, &path).await,
            };
            if call.stop.stopped() {
                return Done::stopped();
            }
            match done {
                Ok((text, said)) => Done::ok(text).said(said),
                Err(text) => Done::error(text).said(said("packages/failed")),
            }
        })
    }
}

impl Texts {
    /// 列表一个一行：读不了的说哪里不对，卸掉的出厂包说卸掉了，关着的说关着。
    fn rows(&self, listed: &Value) -> String {
        let field = |one: &Value, key: &str| one[key].as_str().unwrap_or_default().to_string();
        listed
            .as_array()
            .into_iter()
            .flatten()
            .map(|one| {
                let (package, name, summary) = (
                    field(one, "package"),
                    field(one, "name"),
                    field(one, "summary"),
                );
                let row = [
                    ("package", package.as_str()),
                    ("name", name.as_str()),
                    ("summary", summary.as_str()),
                ];
                let line = if let Some(problem) = one["problem"]
                    .as_str()
                    .filter(|_| one.get("kind").is_none())
                {
                    say(&self.broken, &[("package", &package), ("problem", problem)])
                } else if one["removed"] == true {
                    say(&self.removed, &row[..2])
                } else if one["enabled"] == false {
                    say(&self.off, &row)
                } else {
                    say(&self.listed, &row)
                };
                // 没写说明的：说明那一格空着，不留两个空格、行尾不留空格。
                format!("{}\n", line.replace("  ", " ").trim_end())
            })
            .collect()
    }

    /// 看一个包文件夹：装得上的交回核心看一眼的那一份；装不上的照真装那样说。
    async fn inspect(
        &self,
        port: &dyn miyu_tool::PackagesPort,
        path: &str,
    ) -> Result<(String, Said), String> {
        if !Path::new(path).is_absolute() {
            return Err(say(&self.relative, &[]));
        }
        match port.inspect(PathBuf::from(path)).await {
            // 那一份 JSON 另起一行：模板里换进去的字会转义引号。
            Ok(preview) => Ok((
                format!("{}{preview}\n", say(&self.inspected, &[("path", path)])),
                said("packages/inspected").with("path", path),
            )),
            Err(PackageRefusal {
                problem: Some(problem),
                line,
                ..
            }) => Err(match line {
                Some(line) => say(
                    &self.invalid_line,
                    &[
                        ("path", path),
                        ("line", &line.to_string()),
                        ("problem", &problem),
                    ],
                ),
                None => say(&self.invalid, &[("path", path), ("problem", &problem)]),
            }),
            Err(refusal) => Err(self.refused(&refusal, path)),
        }
    }

    /// 别的拒绝：看的是什么、原因代码。
    fn refused(&self, refusal: &PackageRefusal, what: &str) -> String {
        say(
            &self.refused,
            &[("what", what), ("reason", &refusal.reason)],
        )
    }
}
