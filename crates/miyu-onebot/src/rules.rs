//! 场所规则和出厂数据（`docs/blueprint/onebot.md` 第一条「场所规则和出厂数据」，施工 O-21）：写法、怎么套由群聊内核管
//! （`chat.md` 第一条、第八条、第二条第 12 条），读文件是桥的事（群聊内核在第 2 层，不碰磁盘）。
//!
//! | 什么 | 出厂（资源目录的 `software/onebot/`） | 系统（数据根的 `system/`） |
//! |---|---|---|
//! | 场所规则 | `venues.d/*.toml` | `venues.d/*.toml` |
//! | 出厂参数 | `defaults.toml` | 没有：要改写场所规则 |
//! | 违规词表 | `moderation.txt` | `modules/onebot/moderation.txt`，在的话整份替换出厂的 |
//! | 判官的说明（施工 O-23 下） | `judge/*.txt` 十三份 | 没有：给模型看的字随包走 |
//! | 给她看的事实的模板（施工 O-25 下） | `facts/undelivered.txt` | 没有：同上 |
//! | 桥的工具（施工 O-26，O-31 加平台工具） | `tools/*.json` 的说明，`tool-results/` 答的话 | 没有：同上 |
//!
//! - 出厂的起来时读一次、单独查一次（[`Factory::load`]）：有一条问题就是打包的错，桥起不来；之后放在内存里，跑着不再读
//!   （「施工时定的」第 52 条）。
//! - 系统的和出厂的合起来读（[`load`]，交回 [`Loaded`]）：坏的那一项、那一条规则、那一份文件照群聊内核的规矩丢，别的照用；
//!   读不成的照空的用（「施工时定的」第 50 条）。套到场所上：[`Loaded::at`]。
//! - 什么时候重读（[`Venues`]）：要用时交进当时的时刻，隔够了才看一眼系统的两处变没变，变了整份重读，问题记运行日志；不监视
//!   文件（「施工时定的」第 53 条）。

mod facts;
mod files;
mod judge;
mod tools;

use std::fmt::Display;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use miyu_chat::{File, JudgeTexts, Moderation, Params, Problem, Resolved, Rules, Source, Venue};
use miyu_config::problem::Code;
use miyu_kernel::template::Template;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::{PACKAGE, TARGET};
use files::Stamp;
pub use tools::Tools;
pub(crate) use tools::{FETCH_MEDIA, MUTE, POKE, RECALL, SKIP_REPLY};

/// 场所规则的目录：出厂的在资源目录的 `software/onebot/` 里，系统的在数据根的 `system/` 里。
const VENUES: &str = "venues.d";

/// 出厂参数：只有出厂的（`chat.md` 第八条「怎么走」第 3 条）。
const DEFAULTS: &str = "defaults.toml";

/// 违规词表：出厂的在 `software/onebot/` 里，系统的在 `system/modules/onebot/` 里。
const MODERATION: &str = "moderation.txt";

/// 包自己的系统数据在 `system/` 下的哪个目录里，再往下是包的编号（2026-10-09 核心的主会话定）。
const MODULES: &str = "modules";

/// 出厂的数据：起来时读一次、查过，跑着不再读。
#[derive(Debug)]
pub struct Factory {
    /// 出厂的规则文件：每次读系统的都和它合起来过一遍 [`Rules::parse`]。
    rules: Vec<File>,
    /// 出厂参数。
    params: Params,
    /// 出厂的违规词表：系统没有那一份时用它。
    keywords: Vec<String>,
    /// 判官的说明（施工 O-23 下）：查过的十三份，问判官的任务各拿一份引用。
    judge: Arc<JudgeTexts>,
    /// 退信的模板（施工 O-25 下，`onebot.md`「退信」第 3 条）：查过字段。
    undelivered: Template,
    /// 桥的工具（施工 O-26，`onebot.md`「提供者和不说话」）：说明、答的两句，查过字段。
    tools: Arc<Tools>,
}

impl Factory {
    /// 读资源目录 `resources` 里的出厂数据：规则文件单独过一遍 [`Rules::parse`]（合上系统的以后，被同名替换的那一份不读，
    /// 单独过才查得全），出厂参数过 [`Params::read`]，违规词表过 [`Moderation::parse_keywords`]，判官的说明过
    /// [`JudgeTexts::new`]（施工 O-23 下），退信的模板过 [`Template::parse`]、只认三个字段（施工 O-25 下），桥的工具的说明、
    /// 答的两句照 `miyu_tool::load` 读、查过字段（施工 O-26）。
    ///
    /// # Errors
    ///
    /// 有一条问题就是打包的错（警告也算，照 `chat.md` 第八条施工时定的第 10 条），交回全部问题：规则写错、出厂参数写错、
    /// 判官的说明、退信的模板、桥的工具写坏了、哪一份不在或读不成（`venues.d` 列不出来也是）。问题照规则文件、出厂参数、违规
    /// 词表、判官的说明、退信的模板、桥的工具的先后。
    pub fn load(resources: &ResourceRoot) -> Result<Factory, Vec<Problem>> {
        let dir = resources.path().join("software").join(PACKAGE);
        let mut problems = Vec::new();
        let rules = files::rules(&dir.join(VENUES), VENUES, Source::Factory, &mut problems);
        problems.extend(Rules::parse(&rules).problems);
        let params = match required(&dir.join(DEFAULTS), DEFAULTS, &mut problems) {
            Some(text) => {
                let file = File {
                    source: Source::Factory,
                    name: DEFAULTS.to_string(),
                    text,
                };
                match Params::read(&file) {
                    Ok(params) => Some(params),
                    Err(found) => {
                        problems.extend(found);
                        None
                    }
                }
            }
            None => None,
        };
        let keywords = required(&dir.join(MODERATION), MODERATION, &mut problems)
            .map(|text| Moderation::parse_keywords(&text));
        let judge = judge::texts(&dir, &mut problems);
        let undelivered = facts::undelivered(&dir, &mut problems);
        let tools = tools::load(resources.path(), &mut problems);
        match (params, keywords, judge, undelivered, tools) {
            (Some(params), Some(keywords), Some(judge), Some(undelivered), Some(tools))
                if problems.is_empty() =>
            {
                Ok(Factory {
                    rules,
                    params,
                    keywords,
                    judge: Arc::new(judge),
                    undelivered,
                    tools: Arc::new(tools),
                })
            }
            _ => Err(problems),
        }
    }

    /// 桥的工具（施工 O-26）：出厂的，跑着不再读。跟核心的那一头登记、答请求各拿一份引用。
    pub fn tools(&self) -> Arc<Tools> {
        Arc::clone(&self.tools)
    }
}

/// 出厂的一份 `path`（问题里写成 `name`）：不在的、读不成的记一条问题，交回空的。
fn required(path: &Path, name: &str, problems: &mut Vec<Problem>) -> Option<String> {
    match files::read(path) {
        Ok(Some(text)) => Some(text),
        Ok(None) => {
            let missing = io::Error::from(io::ErrorKind::NotFound).to_string();
            problems.push(files::whole(
                Code::Unreadable,
                Source::Factory,
                name,
                Some(missing),
            ));
            None
        }
        Err(error) => {
            problems.push(files::trouble(error, Source::Factory, name));
            None
        }
    }
}

/// 读好的一份：出厂的合上系统的。
#[derive(Debug)]
pub struct Loaded {
    /// 场所规则：出厂的和系统的合起来，照文件名排好先后。
    pub rules: Rules,
    /// 出厂参数：套场所时再套上规则改的几项（[`Loaded::at`]）。
    pub params: Params,
    /// 用着的违规词表：系统那一份在的是它（读不成的是空的），不在的是出厂的。
    pub keywords: Vec<String>,
    /// 读系统的发现的问题：读不成的和写错的合在一起照文件名排（同一份里照行、列），违规词表的在最后。
    pub problems: Vec<Problem>,
}

/// 套到一个场所上的样子（[`Loaded::at`]）。
#[derive(Debug)]
pub struct Applied {
    /// 规则设到的每一项的值和来处（[`Rules::resolve`]）。
    pub resolved: Resolved,
    /// 出厂参数套上规则改的几项（[`Params::at`]）。
    pub params: Params,
}

impl Loaded {
    /// 套到场所 `venue` 上：先 [`Rules::resolve`]，再 [`Params::at`]（「场所规则和出厂数据」第 5 条）。
    pub fn at(&self, venue: &Venue) -> Applied {
        let resolved = self.rules.resolve(venue);
        let params = self.params.at(&resolved);
        Applied { resolved, params }
    }
}

/// 读数据根 `root` 里系统的那几份，和出厂的 `factory` 合起来（「场所规则和出厂数据」第 2 条）。不会失败：读不成的、写错的
/// 都变成问题，其余照用。不记运行日志（[`Venues`] 记；`venue show` 印出来）。
pub fn load(factory: &Factory, root: &DataRoot) -> Loaded {
    let system = root.system();
    let mut problems = Vec::new();
    let mut files = factory.rules.clone();
    files.extend(files::rules(
        &system.join(VENUES),
        VENUES,
        Source::System,
        &mut problems,
    ));
    let parsed = Rules::parse(&files);
    problems.extend(parsed.problems);
    // 读不成的（整份的）和写错的合在一起照文件名排：读不成的那一份交的是空的字，不会再有写错的；同一份里群聊内核已经照行、
    // 列排好，稳定的排序不打乱。
    problems.sort_by(|one, other| one.file.cmp(&other.file));
    let keywords = match files::read(&words(root)) {
        Ok(Some(text)) => Moderation::parse_keywords(&text),
        Ok(None) => factory.keywords.clone(),
        Err(error) => {
            problems.push(files::trouble(error, Source::System, MODERATION));
            Vec::new()
        }
    };
    Loaded {
        rules: parsed.rules,
        params: factory.params.clone(),
        keywords,
        problems,
    }
}

/// 系统的违规词表：`system/modules/onebot/moderation.txt`。
fn words(root: &DataRoot) -> PathBuf {
    root.system().join(MODULES).join(PACKAGE).join(MODERATION)
}

/// 系统的两处这一刻的样子。
fn stamp(root: &DataRoot) -> Stamp {
    Stamp::of(&root.system().join(VENUES), &words(root))
}

/// 桥手里的场所规则和出厂数据：要用时交进当时的时刻，隔够了才看一眼系统的变没变，变了整份重读（「场所规则和出厂数据」
/// 第 3、4 条）。
#[derive(Debug)]
pub struct Venues {
    /// 出厂的：不再读。
    factory: Factory,
    /// 数据根：系统的在它的 `system/` 里。
    root: DataRoot,
    /// 隔多久才看一眼（`bridge.json` 的 `rules_check_millis`）。
    every: Duration,
    /// 上一次看的时刻。
    looked: Instant,
    /// 上一次看到的样子。
    stamp: Stamp,
    /// 手里的那一份。
    loaded: Loaded,
}

impl Venues {
    /// 读一次系统的（问题记运行日志），记下这一刻 `now`：之后隔 `every` 才再看。
    pub fn new(factory: Factory, root: &DataRoot, every: Duration, now: Instant) -> Venues {
        // 先记样子再读：读的时候又改了的，下一次看得出来。
        let stamp = stamp(root);
        let loaded = reload(&factory, root);
        Venues {
            factory,
            root: root.clone(),
            every,
            looked: now,
            stamp,
            loaded,
        }
    }

    /// 桥的工具（施工 O-31：平台工具的话由跟核心的那一头答）：出厂的，跑着不再读。
    pub fn tools(&self) -> Arc<Tools> {
        self.factory.tools()
    }

    /// 判官的说明（施工 O-23 下）：出厂的，跑着不再读。
    pub fn judge_texts(&self) -> Arc<JudgeTexts> {
        Arc::clone(&self.factory.judge)
    }

    /// 退信的模板（施工 O-25 下）：出厂的，跑着不再读。
    pub fn undelivered(&self) -> &Template {
        &self.factory.undelivered
    }

    /// 这一刻 `now` 该用的那一份：离上一次看不到 `every` 的照手里的；到了，看一眼系统的两处，和上一次的一样照手里的，
    /// 不一样整份重读（问题记运行日志）。
    pub fn current(&mut self, now: Instant) -> &Loaded {
        if now.saturating_duration_since(self.looked) >= self.every {
            self.looked = now;
            let stamp = stamp(&self.root);
            if stamp != self.stamp {
                self.stamp = stamp;
                self.loaded = reload(&self.factory, &self.root);
            }
        }
        &self.loaded
    }
}

/// 读一次系统的，每条问题记一行运行日志（第 4 条），读完记一行。
fn reload(factory: &Factory, root: &DataRoot) -> Loaded {
    let loaded = load(factory, root);
    for problem in &loaded.problems {
        tracing::warn!(
            target: TARGET,
            code = problem.code.as_str(),
            source = ?problem.source,
            file = %problem.file,
            rule = %shown(problem.rule),
            key = %shown(problem.key.as_deref()),
            line = %shown(problem.at.map(|at| at.line)),
            got = %shown(problem.got.as_deref()),
            why = %shown(problem.why.as_deref()),
            "venue rules problem"
        );
    }
    tracing::info!(target: TARGET, problems = loaded.problems.len(), "venue rules read");
    loaded
}

/// 可能没有的一格写进运行日志：没有的是空的。
fn shown(value: Option<impl Display>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}
