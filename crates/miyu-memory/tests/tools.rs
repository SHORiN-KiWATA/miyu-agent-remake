//! 记忆的三件工具（施工 R-3 中，`docs/blueprint/memory.md`「工具」）：参数、长度、类、编号，端口交回的几种，结果的句子逐字节
//! 比，给人看的说法。端口是假的：真的在 `miyu-session` 里测。

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::Block;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::MemoryId;
use miyu_tool::{
    Call, Done, FoundMemory, FoundTurn, MemoryPort, Pending, Progress, Refused, Remember, Searched,
    Stop, Tool,
};

/// 一个假端口：记下她要做的，照预先定好的交回。
#[derive(Default)]
struct Fake {
    saved: Mutex<Vec<Remember>>,
    retired: Mutex<Vec<(MemoryId, String)>>,
    searched: Mutex<Vec<(String, bool)>>,
    refuse: Option<Refused>,
    found: Searched,
}

impl MemoryPort for Fake {
    fn save<'a>(&'a self, remember: Remember) -> Pending<'a, Result<MemoryId, Refused>> {
        self.saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(remember);
        let answer = match &self.refuse {
            Some(refused) => Err(refused.clone()),
            None => Ok(id(3)),
        };
        Box::pin(async move { answer })
    }

    fn retire<'a>(&'a self, id: MemoryId, why: String) -> Pending<'a, Result<(), Refused>> {
        self.retired
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((id, why));
        let answer = self.refuse.clone().map_or(Ok(()), Err);
        Box::pin(async move { answer })
    }

    fn search<'a>(
        &'a self,
        query: String,
        forgotten: bool,
    ) -> Pending<'a, Result<Searched, String>> {
        self.searched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((query, forgotten));
        let found = self.found.clone();
        Box::pin(async move { Ok(found) })
    }
}

fn id(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).expect("从 1 起"))
}

fn at(day: u32) -> Timestamp {
    Timestamp::parse(&format!("2026-10-{day:02}T08:00:00.000Z")).expect("合写法")
}

fn tools() -> Vec<Arc<dyn Tool>> {
    miyu_memory::tools(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
        .expect("读得出")
}

/// 用假端口 `port`（没有的是记忆没开）调一次 `name`。
async fn call(port: Option<Arc<Fake>>, name: &str, args: serde_json::Value) -> Done {
    let tool = tools()
        .into_iter()
        .find(|tool| tool.spec().name == name)
        .expect("有这件");
    let call = Call {
        args: args.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Arc::default(),
        stop: Stop::default(),
        sandbox: None,
        log: None,
        offset: UtcOffset::from_minutes(0).expect("零时区"),
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
        questions: None,
        memory: port.map(|port| port as Arc<dyn MemoryPort>),
        packages: None,
        ids: None,
    };
    tool.run(call, Progress::new(|_| {})).await
}

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

fn said(done: &Done) -> String {
    done.human
        .as_ref()
        .map(|said| said.key.clone())
        .unwrap_or_default()
}

#[tokio::test]
async fn the_three_tools_are_there_and_only_read() {
    let names: Vec<String> = tools()
        .iter()
        .map(|tool| tool.spec().name.clone())
        .collect();
    assert_eq!(names, ["remember", "forget", "memory_search"]);
    for tool in tools() {
        assert_eq!(
            tool.spec().access,
            miyu_kernel::tool::Access::Read,
            "{}",
            tool.spec().name
        );
    }
}

#[tokio::test]
async fn remembering_saves_and_says_the_id() {
    let port = Arc::new(Fake::default());
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"class":"user","text":"用户用 N 卡"}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Saved as m3.");
    assert_eq!(said(&done), "software/memory/remember/saved");
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"class":"feedback","text":"别叫我主人，听着别扭","replaces":"m1"}),
    )
    .await;
    assert_eq!(text(&done), "Saved as m3, replacing m1.");
    let saved = port
        .saved
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    assert_eq!(
        saved,
        [
            Remember {
                class: "user".into(),
                text: "用户用 N 卡".into(),
                replaces: None
            },
            Remember {
                class: "feedback".into(),
                text: "别叫我主人，听着别扭".into(),
                replaces: Some(id(1))
            },
        ]
    );
}

#[tokio::test]
async fn too_long_or_an_unknown_class_is_not_saved() {
    let port = Arc::new(Fake::default());
    let long = "猫".repeat(121);
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"class":"user","text":long}),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "That is 121 characters; keep a memory under 120."
    );
    let exact = "猫".repeat(120);
    assert!(
        !call(
            Some(Arc::clone(&port)),
            "remember",
            serde_json::json!({"class":"user","text":exact})
        )
        .await
        .error
    );
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"class":"mood","text":"开心"}),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Unknown class mood; use user, feedback, episode or reference."
    );
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"text":"开心"}),
    )
    .await;
    assert!(done.error);
    assert!(
        text(&done).starts_with("The arguments are not right: "),
        "{}",
        text(&done)
    );
    let done = call(
        Some(Arc::clone(&port)),
        "remember",
        serde_json::json!({"class":"user","text":"x","replaces":"12"}),
    )
    .await;
    assert_eq!(
        (done.error, text(&done)),
        (true, "There is no memory 12.".to_string())
    );
    assert_eq!(
        port.saved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len(),
        1,
        "只有刚好 120 字那条记下了"
    );
}

#[tokio::test]
async fn refusals_from_the_port_are_said() {
    for (refused, expected) in [
        (Refused::NoSuch(id(9)), "There is no memory m9."),
        (
            Refused::NotCurrent(id(9)),
            "m9 was already replaced or retired.",
        ),
        (
            Refused::Failed("disk full".into()),
            "Memory is not available right now: disk full.",
        ),
    ] {
        let port = Arc::new(Fake {
            refuse: Some(refused.clone()),
            ..Fake::default()
        });
        let done = call(
            Some(Arc::clone(&port)),
            "remember",
            serde_json::json!({"class":"user","text":"x","replaces":"m9"}),
        )
        .await;
        assert_eq!(
            (done.error, text(&done)),
            (true, expected.to_string()),
            "{refused:?}"
        );
        let done = call(
            Some(port),
            "forget",
            serde_json::json!({"id":"m9","why":"说错了"}),
        )
        .await;
        assert_eq!(
            (done.error, text(&done)),
            (true, expected.to_string()),
            "{refused:?}"
        );
    }
}

#[tokio::test]
async fn forgetting_retires_and_says_so() {
    let port = Arc::new(Fake::default());
    let done = call(
        Some(Arc::clone(&port)),
        "forget",
        serde_json::json!({"id":"m4","why":"用户说不养猫了"}),
    )
    .await;
    assert_eq!(
        (done.error, text(&done)),
        (false, "Retired m4.".to_string())
    );
    assert_eq!(said(&done), "software/memory/forget/retired");
    assert_eq!(
        *port.retired.lock().unwrap_or_else(PoisonError::into_inner),
        [(id(4), "用户说不养猫了".to_string())]
    );
    let done = call(
        Some(port),
        "forget",
        serde_json::json!({"id":"four","why":"?"}),
    )
    .await;
    assert_eq!(
        (done.error, text(&done)),
        (true, "There is no memory four.".to_string())
    );
}

#[tokio::test]
async fn searching_lists_memories_then_past_conversations() {
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法");
    let long = format!("{}尾巴", "长".repeat(300));
    let port = Arc::new(Fake {
        found: Searched {
            memories: vec![
                FoundMemory {
                    id: id(3),
                    class: "user".into(),
                    text: "用户用 N 卡".into(),
                    at: at(5),
                    retired: false,
                },
                FoundMemory {
                    id: id(1),
                    class: "user".into(),
                    text: "用户用 A 卡".into(),
                    at: at(1),
                    retired: true,
                },
            ],
            turns: vec![
                FoundTurn {
                    session: session.clone(),
                    turn: TurnId::new(Seq::new(8).expect("从 1 起")),
                    at: at(6),
                    text: "我换了显卡\n\n好的".into(),
                },
                FoundTurn {
                    session,
                    turn: TurnId::new(Seq::new(12).expect("从 1 起")),
                    at: at(6),
                    text: long,
                },
            ],
        },
        ..Fake::default()
    });
    let done = call(
        Some(Arc::clone(&port)),
        "memory_search",
        serde_json::json!({"query":"显卡","forgotten":true}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        format!(
            "m3 user 2026-10-05: 用户用 N 卡\nm1 user 2026-10-01 (retired): 用户用 A 卡\n2026-10-06 session 22334455: 我换了显卡 / 好的\n2026-10-06 session 22334455: {}…",
            "长".repeat(300)
        )
    );
    assert_eq!(
        *port.searched.lock().unwrap_or_else(PoisonError::into_inner),
        [("显卡".to_string(), true)]
    );
    let empty = Arc::new(Fake::default());
    let done = call(
        Some(Arc::clone(&empty)),
        "memory_search",
        serde_json::json!({"query":"樱花"}),
    )
    .await;
    assert_eq!(
        (done.error, text(&done)),
        (false, "Nothing found.".to_string())
    );
    assert_eq!(
        *empty
            .searched
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        [("樱花".to_string(), false)]
    );
}

#[tokio::test]
async fn without_memory_the_tools_say_it_is_off() {
    for (name, args) in [
        ("remember", serde_json::json!({"class":"user","text":"x"})),
        ("forget", serde_json::json!({"id":"m1","why":"x"})),
        ("memory_search", serde_json::json!({"query":"x"})),
    ] {
        let done = call(None, name, args).await;
        assert_eq!(
            (done.error, text(&done)),
            (true, "Memory is off in this session.".to_string()),
            "{name}"
        );
    }
}
