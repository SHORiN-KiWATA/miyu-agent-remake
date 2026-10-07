//! 拼快照（`docs/designs/26-提示词.md` 第四节「怎么拼」）：system 照第四节的先后排，每一块去掉末尾的
//! 空白，块和块之间空一行，没有的块不留空行。
//!
//! 施工 3-6（上）时只有人设。别的块跟着各自的功能来，按 J12 先实测证明不加不行：场所说明施工 7-5 加（子会话）；核心的
//! 几行施工 2-7 补加，权限那一行和本机文件的路径那一行，2026-10-01 主会话 A/B 实测过（`26-提示词.md` 第十节）。

use miyu_kernel::id::ContentHash;

use crate::pause::PAUSE;
use crate::persona::Demo;
use crate::rebuild::REBUILD;
use crate::recap::RECAP;
use crate::shorten::SHORTEN;
use crate::snapshot::{CompactionNumbers, CoreTexts, Snapshot, TAIL};

/// 有计划的重启打断了一轮，再起来时连着接着干几次：`02-内核.md` 第六节「载入、崩溃、重启」的初值。
const RESUMES: u32 = 3;

/// 压缩用的数的出厂值（`compaction.md`「对外的样子」）：输出预留的上限 20000、余量 13000（照 Claude Code），
/// 一张图、一个文件各算 2000，尾巴至多 16000（2026-09-29 项目主人定）；压后重建、熔断、截短重试照各自的出厂数。
const COMPACTION: CompactionNumbers = CompactionNumbers {
    reserve_cap: 20_000,
    margin: 13_000,
    image: 2_000,
    file: 2_000,
    tail: TAIL,
    rebuild: Some(REBUILD),
    pause: Some(PAUSE),
    shorten: Some(SHORTEN),
};

/// 读好的原文：随核心附带的字，和这个人格的字。执行器从资源目录读（`miyu-store` 的资源目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    /// 随核心附带的字（`resources/core/`）。
    pub core: CoreTexts,
    /// 这个人格的字。
    pub persona: PersonaTexts,
    /// 角色扮演提示的包装（`core/facts/reminder-open.txt`、`reminder-close.txt`，施工 P-1 补）：拼进快照的 `reminder`，
    /// 不另存，没有角色扮演提示的快照字节不变。
    pub reminder: Wrap,
}

/// 包一段字的开头、收尾。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wrap {
    /// 开头。
    pub open: String,
    /// 收尾。
    pub close: String,
}

/// 一个人格的字（`<人格目录>/prompts/`，施工 P-1 上照几层叠好）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaTexts {
    /// 人设（`persona.md`）：没有的是空的。
    pub persona: String,
    /// 示范对话（`examples.md`，`crate::persona::read_examples` 读好的）：没有的是空的。
    pub examples: Vec<Demo>,
    /// 角色扮演提示（`reminders.md`，施工 P-1 补）：整份是一条；没有的是空的。
    pub reminders: String,
}

/// 照 `26-提示词.md` 第四节拼出人格 `persona` 的快照。`attended` 是这个场所有没有人能确认。
pub fn compose(persona: &str, sources: Sources, attended: bool) -> Snapshot {
    let digest = sources.persona.digest();
    Snapshot {
        persona: persona.to_string(),
        system: system(&[&sources.persona.persona]),
        demos: sources.persona.examples,
        tools: Vec::new(),
        core: sources.core,
        step_limit: None,
        attended,
        resumes: RESUMES,
        compaction: Some(COMPACTION),
        jobs: Some(crate::jobs::JOB_NUMBERS),
        recap: Some(RECAP),
        title: Some(crate::title::TITLE),
        peers: Some(crate::peers::PEERS),
        memory: None,
        reminder: reminder(&sources.persona.reminders, &sources.reminder),
        persona_digest: Some(digest),
    }
}

impl PersonaTexts {
    /// 三份字的指纹（施工 P-1 再补）：拼进快照的 `persona_digest`，回合开始时执行器照它认出人格的文件改了。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：字和示范对话总写得成 JSON。
    pub fn digest(&self) -> ContentHash {
        let texts = (&self.persona, &self.examples, &self.reminders);
        ContentHash::of(&serde_json::to_vec(&texts).expect("字和示范对话写得成 JSON"))
    }
}

/// 角色扮演提示拼成的一块（施工 P-1 补）：开头、去掉末尾空白的原文、换行、收尾。原文不转义：是人格的作者写的。去掉末尾
/// 空白以后是空的，没有。
fn reminder(text: &str, wrap: &Wrap) -> Option<String> {
    let text = text.trim_end();
    (!text.is_empty()).then(|| format!("{}{text}\n{}", wrap.open, wrap.close))
}

/// 核心的几行（`26-提示词.md` 第四节第 3 块，施工 2-7 补）和风格锁（第 7 块，施工 P-1 补）：执行器从资源目录读好交进来，
/// 造会话时拼进 system。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLines {
    /// `<permission>` 那一块怎么读、每一级能做什么、只有人能切（`core/permission-rule.txt`）。没有工具的会话不带。
    pub permission: String,
    /// 回答里提到本机的文件写绝对路径（`core/local-paths-rule.txt`）。
    pub local_paths: String,
    /// 风格锁（`core/style-lock.txt`）：只有带角色扮演提示的人格带（[`Snapshot::with_style_lock`]）。
    pub style_lock: String,
}

impl Snapshot {
    /// 带上核心的几行（施工 2-7 补）：system 的第三块，接在人设、场所说明后面，所以在 [`Snapshot::with_tools`]、
    /// [`Snapshot::with_venue`] 以后最后调。块里一行一句，先权限、后本机文件的路径；工具面是空的会话用不上权限那一句，
    /// 不带（26 第十节）。以前造的快照 system 里没有这一块，载入照快照发，前缀一字不变。
    #[must_use]
    pub fn with_core_lines(mut self, lines: &CoreLines) -> Snapshot {
        let permission = (!self.tools.is_empty()).then_some(lines.permission.as_str());
        let block = permission
            .into_iter()
            .chain([lines.local_paths.as_str()])
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        self.system = system(&[&self.system, &block]);
        self
    }

    /// 能不能换成 `new`（施工 P-1 再补）：除了 system、示范对话、角色扮演提示和人格的指纹，别的格都一样。不一样的说明程序
    /// 升级过、执行器照旧快照造的那几份字（驱动的占位、权限策略的几句）还是旧的，换一半会让两版字混着用。
    pub fn swappable(&self, new: &Snapshot) -> bool {
        let rest = |snapshot: &Snapshot| Snapshot {
            system: String::new(),
            demos: Vec::new(),
            reminder: None,
            persona_digest: None,
            ..snapshot.clone()
        };
        rest(self) == rest(new)
    }

    /// 带上风格锁（施工 P-1 补）：system 的最后一块（26 第四节第 7 块），在 [`Snapshot::with_core_lines`] 以后调。只有带
    /// 角色扮演提示的人格带（2026-10-07 项目主人定），别的 system 一字不变。
    #[must_use]
    pub fn with_style_lock(mut self, lock: &str) -> Snapshot {
        if self.reminder.is_some() {
            self.system = system(&[&self.system, lock]);
        }
        self
    }

    /// 带上场所说明（施工 7-5）：system 的第二块，接在人设后面（26 第四节）。现在只有子会话有，原文是
    /// `core/jobs/subagent-venue.txt`（`agents.md` 第九条第 3 条）；照拼 system 的规矩去掉末尾的空白、空一行。
    #[must_use]
    pub fn with_venue(mut self, venue: &str) -> Snapshot {
        self.system = system(&[&self.system, venue]);
        self
    }
}

/// system：照先后，每一块去掉末尾的空白，没有的块不留空行，块和块之间空一行。
fn system(pieces: &[&str]) -> String {
    pieces
        .iter()
        .map(|piece| piece.trim_end())
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_are_trimmed_and_joined_with_a_blank_line() {
        assert_eq!(system(&["人设\n", "", "  \n", "场所\n\n"]), "人设\n\n场所");
        assert_eq!(
            system(&["You are a helpful software engineer.\n"]),
            "You are a helpful software engineer."
        );
        assert_eq!(system(&[]), "");
        // 开头的空白是人格自己写的，照留。
        assert_eq!(system(&["  缩进的第一行\n"]), "  缩进的第一行");
    }

    #[test]
    fn the_venue_note_follows_the_persona_after_a_blank_line() {
        let snapshot = crate::test_support::engineer();
        let persona = snapshot.system.clone();
        let child = snapshot.with_venue("You are a subagent.\n");
        assert_eq!(child.system, format!("{persona}\n\nYou are a subagent."));
        let blank = crate::test_support::engineer().with_venue("\n");
        assert_eq!(blank.system, persona, "空的说明不留空行");
    }

    /// 核心的几行（施工 2-7 补）：两句，一行一句，排在人设、场所说明后面，空一行。
    fn lines() -> CoreLines {
        CoreLines {
            permission: "Permission rule.\n".to_string(),
            local_paths: "Local paths rule.\n".to_string(),
            style_lock: String::new(),
        }
    }

    /// 一件工具：有它，工具面就不是空的。
    fn a_tool() -> crate::tools::ToolEntry {
        serde_json::from_str(r#"{"name":"read","description":"Read.","parameters":{"type":"object"},"access":"read"}"#)
            .unwrap()
    }

    #[test]
    fn the_core_lines_come_after_the_persona_and_the_venue() {
        let persona = crate::test_support::engineer().system;
        let main = crate::test_support::engineer()
            .with_tools(vec![a_tool()])
            .with_core_lines(&lines());
        assert_eq!(
            main.system,
            format!("{persona}\n\nPermission rule.\nLocal paths rule.")
        );
        let child = crate::test_support::engineer()
            .with_tools(vec![a_tool()])
            .with_venue("You are a subagent.\n")
            .with_core_lines(&lines());
        assert_eq!(
            child.system,
            format!("{persona}\n\nYou are a subagent.\n\nPermission rule.\nLocal paths rule.")
        );
    }

    #[test]
    fn a_session_without_tools_has_no_permission_line() {
        let persona = crate::test_support::engineer().system;
        let bare = crate::test_support::engineer().with_core_lines(&lines());
        assert_eq!(bare.system, format!("{persona}\n\nLocal paths rule."));
    }

    /// 带角色扮演提示的人格：快照里是拼好的一块（包装、去掉末尾空白的原文、换行、收尾），造出的策略第一轮就注入它。
    fn reminding(reminders: &str) -> Snapshot {
        let mut sources = crate::test_support::sources();
        sources.persona.reminders = reminders.to_string();
        sources.reminder = Wrap {
            open: "<persona-reminder>\n".to_string(),
            close: "</persona-reminder>\n".to_string(),
        };
        compose("miyu", sources, true)
    }

    #[test]
    fn a_reminder_is_wrapped_into_the_snapshot_and_reaches_the_facts() {
        let snapshot = reminding("  Stay soft.\n\n");
        let block = "<persona-reminder>\n  Stay soft.\n</persona-reminder>\n";
        assert_eq!(snapshot.reminder.as_deref(), Some(block));
        let fact = snapshot
            .policy()
            .unwrap()
            .facts
            .reminder(&miyu_kernel::history::History::default())
            .unwrap();
        assert_eq!(fact.text, block);
        let read = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
        assert_eq!(read, snapshot, "存得回来");
    }

    #[test]
    fn a_blank_reminder_is_none_and_the_snapshot_is_as_before() {
        assert_eq!(reminding(" \n\t\n").reminder, None);
        let engineer = crate::test_support::engineer();
        assert_eq!(engineer.reminder, None);
        let bytes = String::from_utf8(engineer.to_bytes()).unwrap();
        assert!(!bytes.contains("reminder"), "没有的不写：{bytes}");
        assert!(
            engineer
                .policy()
                .unwrap()
                .facts
                .reminder(&miyu_kernel::history::History::default())
                .is_none()
        );
    }

    #[test]
    fn the_style_lock_ends_the_system_of_a_persona_with_a_reminder_only() {
        let lock = "<style-lock>Stay.</style-lock>\n";
        let main = reminding("Stay soft.")
            .with_tools(vec![a_tool()])
            .with_core_lines(&lines())
            .with_style_lock(lock);
        assert!(
            main.system
                .ends_with("Permission rule.\nLocal paths rule.\n\n<style-lock>Stay.</style-lock>"),
            "{}",
            main.system
        );
        let plain = crate::test_support::engineer().with_core_lines(&lines());
        assert_eq!(
            plain.clone().with_style_lock(lock).system,
            plain.system,
            "没有角色扮演提示的不带"
        );
    }

    /// 人格的指纹（施工 P-1 再补）：同样的字同样的指纹，三份里改了哪一份都变；拼进快照。
    #[test]
    fn the_digest_follows_the_three_persona_texts() {
        let base = crate::test_support::sources().persona;
        let digest = base.digest();
        assert_eq!(base.clone().digest(), digest);
        let mut persona = base.clone();
        persona.persona.push('x');
        let mut examples = base.clone();
        examples.examples = crate::persona::read_examples("user: a\nassistant: b\n").unwrap();
        let mut reminders = base.clone();
        reminders.reminders = "Stay.".to_string();
        for changed in [persona, examples, reminders] {
            assert_ne!(changed.digest(), digest);
        }
        assert_eq!(crate::test_support::engineer().persona_digest, Some(digest));
    }

    /// 换快照只许人格的那几格不一样（施工 P-1 再补）：核心的字、工具面、人格编号变了的都不算。
    #[test]
    fn only_the_persona_parts_may_differ_for_a_swap() {
        let old = crate::test_support::engineer()
            .with_tools(vec![a_tool()])
            .with_core_lines(&lines());
        let mut sources = crate::test_support::sources();
        sources.persona.persona = "You are Miyu.\n".to_string();
        sources.persona.reminders = "Stay.".to_string();
        let new = compose("engineer", sources, true)
            .with_tools(vec![a_tool()])
            .with_core_lines(&lines())
            .with_style_lock("<lock/>");
        assert!(old.swappable(&new));
        let mut core = new.clone();
        core.core.facts.env = "<e/>\n".to_string();
        assert!(!old.swappable(&core), "核心的字变了");
        let mut tools = new.clone();
        tools.tools.clear();
        assert!(!old.swappable(&tools), "工具面变了");
        let mut other = new;
        other.persona = "miyu".to_string();
        assert!(!old.swappable(&other), "不是同一个人格");
    }

    #[test]
    fn without_the_core_lines_the_system_is_as_before() {
        let tooled = crate::test_support::engineer().with_tools(vec![a_tool()]);
        assert_eq!(tooled.system, "You are a helpful software engineer.");
        let empty = CoreLines {
            permission: "\n".to_string(),
            local_paths: String::new(),
            style_lock: String::new(),
        };
        assert_eq!(
            tooled.clone().with_core_lines(&empty).system,
            tooled.system,
            "空的几行不留空行"
        );
    }
}
