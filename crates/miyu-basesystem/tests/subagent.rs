//! `subagent`（`docs/blueprint/tools/subagent.md`，施工 7-5，7-5 再补改名）：声明标题、交代和挡位（施工 8-8）；经端口派出去，
//! 交回编号和标题、报 `job.started`；没有端口、端口派不了的照「派不了」出错；参数不对的照共用的那一句。挡位只认四个，交给
//! 端口，不写的交没有，写错的照参数不对、端口一次都不问。给人看的说法两种语言都换得出字；显示名新旧两个名字都有、三种语言
//! 里一样。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{JobKind, JobStarted};
use miyu_kernel::id::{JobId, SessionId};
use miyu_kernel::tool::Access;
use miyu_tool::{AgentPort, Done, Effect, NotSpawned, Spawned, Spawning};

use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use support::{Site, check, human, readable, resources, said, tool};

/// 子会话的编号。
const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 假的端口：记下交给它的每一次标题和交代、挡位（施工 8-8）；`session` 有的派得出去，编号照第几次派从 `j1` 数起，没有的
/// 一律派不了。
struct Port {
    session: Option<SessionId>,
    asked: Mutex<Vec<(String, String)>>,
    tiers: Mutex<Vec<Option<String>>>,
}

impl Port {
    fn new(session: Option<&str>) -> Arc<Port> {
        Arc::new(Port {
            session: session.map(|session| SessionId::parse(session).expect("会话编号合写法")),
            asked: Mutex::new(Vec::new()),
            tiers: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<(String, String)> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn tiers(&self) -> Vec<Option<String>> {
        self.tiers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AgentPort for Port {
    fn spawn<'a>(
        &'a self,
        description: &'a str,
        prompt: &'a str,
        tier: Option<&'a str>,
    ) -> Spawning<'a> {
        let mut asked = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        asked.push((description.to_string(), prompt.to_string()));
        self.tiers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tier.map(str::to_string));
        let job = JobId::new(asked.len() as u64).expect("从 1 数起");
        let answer = match &self.session {
            Some(session) => Ok(Spawned {
                job,
                session: session.clone(),
            }),
            None => Err(NotSpawned),
        };
        Box::pin(async move { answer })
    }
}

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

#[test]
fn it_declares_a_title_the_task_and_a_tier() {
    let subagent = tool("subagent");
    let spec = subagent.spec();
    assert_eq!(spec.name, "subagent");
    // 派出去这一下什么都不改：一步里调几次，读的连着一起派；只读的时候也派得出去，子会话抄着只读。
    assert_eq!(spec.access, Access::Read);
    let parameters: serde_json::Value = serde_json::from_str(spec.parameters.get()).unwrap();
    let names: Vec<&String> = parameters["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(names, ["description", "prompt", "tier"]);
    assert_eq!(parameters["required"], json!(["description", "prompt"]));
    assert_eq!(
        parameters["properties"]["tier"]["enum"],
        json!(miyu_models::reference::TIERS),
        "挡位的说明和代码里认的是同四个"
    );
    assert!(
        spec.description
            .contains("It sees nothing of this conversation"),
        "交代要自己说得清那一句留着：{}",
        spec.description
    );
}

#[tokio::test]
async fn it_hands_the_task_to_the_port_and_reports_the_job() {
    let site = Site::new();
    let port = Port::new(Some(CHILD));
    let prompt = "Read src/lib.rs and tell me what it exports.\nOnly the public items.";
    let done = site
        .done_with_agents(
            "subagent",
            json!({"description": "查导出", "prompt": prompt}),
            Some(port.clone()),
        )
        .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Started subagent j1: \"查导出\".\n");
    assert_eq!(
        port.asked(),
        [("查导出".to_string(), prompt.to_string())],
        "标题、交代原样交出去"
    );
    assert_eq!(
        done.effects,
        [Effect::JobStarted(JobStarted {
            job: JobId::new(1).unwrap(),
            what: JobKind::Agent,
            title: "查导出".to_string(),
            session: Some(SessionId::parse(CHILD).unwrap()),
        })]
    );
}

#[tokio::test]
async fn without_a_port_or_when_the_port_fails_it_is_not_started() {
    let site = Site::new();
    let args = json!({"description": "查导出", "prompt": "Read src/lib.rs."});
    let failing = Port::new(None);
    for agents in [None, Some(failing.clone() as Arc<dyn AgentPort>)] {
        let done = site
            .done_with_agents("subagent", args.clone(), agents)
            .await;
        assert!(done.error);
        assert_eq!(text(&done), "The subagent could not be started.\n");
        assert!(done.effects.is_empty(), "没派出去，没有 job.started");
    }
    assert_eq!(failing.asked().len(), 1, "端口照样问过一次");
}

/// 挡位（施工 8-8）：四个都交给端口，不写、写 `null` 的交没有；写错的（别的词、大小写不对、不是字）照参数不对，端口一次都
/// 不问。
#[tokio::test]
async fn the_tier_is_one_of_four_and_goes_to_the_port() {
    let site = Site::new();
    let port = Port::new(Some(CHILD));
    let task =
        |tier: serde_json::Value| json!({"description": "查导出", "prompt": "Read.", "tier": tier});
    for tier in ["lite", "cheap", "standard", "flagship"] {
        let done = site
            .done_with_agents("subagent", task(json!(tier)), Some(port.clone()))
            .await;
        assert!(!done.error, "{tier}");
    }
    for args in [
        json!({"description": "查导出", "prompt": "Read."}),
        task(serde_json::Value::Null),
    ] {
        let done = site
            .done_with_agents("subagent", args, Some(port.clone()))
            .await;
        assert!(!done.error);
    }
    let some = |tier: &str| Some(tier.to_string());
    assert_eq!(
        port.tiers(),
        [
            some("lite"),
            some("cheap"),
            some("standard"),
            some("flagship"),
            None,
            None
        ]
    );
    for bad in [json!("huge"), json!("Lite"), json!(""), json!(1)] {
        let done = site
            .done_with_agents("subagent", task(bad.clone()), Some(port.clone()))
            .await;
        assert!(done.error, "{bad}");
        assert!(
            text(&done).starts_with("The arguments are not right: "),
            "{}",
            text(&done)
        );
    }
    assert_eq!(port.tiers().len(), 6, "写错的端口不问");
    let done = site
        .done_with_agents("subagent", task(json!("huge")), Some(port.clone()))
        .await;
    assert!(
        text(&done).contains("lite") && text(&done).contains("flagship"),
        "原话列出能写的几个：{}",
        text(&done)
    );
}

#[tokio::test]
async fn without_the_task_nothing_is_asked() {
    let site = Site::new();
    let port = Port::new(Some(CHILD));
    for args in [json!({"description": "查导出"}), json!({"prompt": "Read."})] {
        let done = site
            .done_with_agents("subagent", args, Some(port.clone()))
            .await;
        assert!(done.error);
        assert!(text(&done).starts_with("The arguments are not right: "));
    }
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn every_outcome_says_something_people_can_read() {
    let site = Site::new();
    let mut checked = Vec::new();
    let args = json!({"description": "查导出", "prompt": "Read src/lib.rs."});
    let done = site
        .done_with_agents("subagent", args.clone(), Some(Port::new(Some(CHILD))))
        .await;
    check(
        &mut checked,
        human(done),
        said("agent/started")
            .with("job", "j1")
            .with("title", "查导出"),
    );
    let done = site.done_with_agents("subagent", args, None).await;
    check(&mut checked, human(done), said("agent/not-started"));
    readable(&checked, &["subagent", "agent"]);
}

/// 给人看的显示名：新名字 `subagent`、以前的名字 `agent` 两个键都在，三种语言里各是同一个样子（施工 7-5 再补）：以前造的
/// 会话里调的是 `agent`，头照它找显示名。
#[test]
fn both_names_show_the_same_to_people() {
    let root = ResourceRoot::at(resources());
    for language in ["zh", "en", "ja"] {
        let words = Human::load(&root, language).expect("给人看的字读得出来");
        let now = words.tool("subagent").expect("有 subagent 的显示名");
        assert_eq!(words.tool("agent"), Some(now), "{language}");
    }
}
