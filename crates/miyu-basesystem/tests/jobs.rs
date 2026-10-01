//! `jobs`（`docs/blueprint/tools/jobs.md`，施工 7-4）：三个动作的输出逐字节比；读输出照 `read` 分页；停交给端口，没有、结束了
//! 的出错；参数不对的照共用的那一句。给人看的说法两种语言都换得出字。

mod support;

use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::JobKind;
use miyu_kernel::id::JobId;
use miyu_kernel::tool::Access;
use miyu_tool::{Asking, Background, Done, JobError, JobPort, Listed, Output};

use support::{Site, check, human, readable, said, tool};

/// 假的任务端口：列出来的照给的；`j1` 是后台命令、`j2` 是子代理，输出照给的；停 `j1` 停得了，`j2` 已经结束了，别的没有。
struct Port {
    listed: Vec<Listed>,
    command: Option<String>,
    running: bool,
    reply: Option<String>,
    doing: Vec<String>,
    stopped: Mutex<Vec<JobId>>,
}

impl Port {
    fn new() -> Port {
        Port {
            listed: Vec::new(),
            command: None,
            running: false,
            reply: None,
            doing: Vec::new(),
            stopped: Mutex::new(Vec::new()),
        }
    }

    fn stopped(&self) -> Vec<JobId> {
        self.stopped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl JobPort for Port {
    fn start(&self, _command: Background) -> std::io::Result<JobId> {
        Err(std::io::Error::other("not here"))
    }

    fn list(&self) -> Vec<Listed> {
        self.listed.clone()
    }

    fn output(&self, job: JobId) -> Asking<'_, Result<Output, JobError>> {
        let reader = |text: &Option<String>| {
            text.clone()
                .map(|text| Box::new(Cursor::new(text)) as Box<dyn Read + Send>)
        };
        let output = match job.to_string().as_str() {
            "j1" => Ok(Output {
                what: JobKind::Command,
                text: reader(&self.command),
                running: self.running,
                doing: Vec::new(),
            }),
            "j2" => Ok(Output {
                what: JobKind::Agent,
                text: reader(&self.reply),
                running: self.running,
                doing: self.doing.clone(),
            }),
            _ => Err(JobError::Unknown),
        };
        Box::pin(async move { output })
    }

    fn stop(&self, job: JobId) -> Asking<'_, Result<(), JobError>> {
        let result = match job.to_string().as_str() {
            "j1" => Ok(()),
            "j2" => Err(JobError::Ended),
            _ => Err(JobError::Unknown),
        };
        self.stopped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(job);
        Box::pin(async move { result })
    }
}

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

/// 照 `port` 调一次 `jobs`。
async fn jobs(port: Option<Port>, args: serde_json::Value) -> Done {
    let port = port.map(|port| Arc::new(port) as Arc<dyn JobPort>);
    Site::new().done_jobs("jobs", args, port, None).await
}

fn listed(job: u64, what: JobKind, title: &str, ended: Option<&str>, took_ms: u64) -> Listed {
    Listed {
        job: JobId::new(job).expect("从 1 数起"),
        what,
        title: title.to_string(),
        ended: ended.map(str::to_string),
        took_ms,
    }
}

#[test]
fn it_declares_the_three_actions_and_says_not_to_poll() {
    let jobs = tool("jobs");
    let spec = jobs.spec();
    assert_eq!(spec.name, "jobs");
    // 列、读什么都不改；停掉的是她自己派出去的，只读的时候也停得了。
    assert_eq!(spec.access, Access::Read);
    let parameters: serde_json::Value = serde_json::from_str(spec.parameters.get()).unwrap();
    let names: Vec<&String> = parameters["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(names, ["action", "id", "offset"]);
    assert_eq!(
        parameters["properties"]["action"]["enum"],
        json!(["list", "output", "stop"])
    );
    assert_eq!(parameters["required"], json!(["action"]));
    assert!(
        spec.description.contains("no need to poll"),
        "做完会自己报、不用轮询那一句留着：{}",
        spec.description
    );
}

#[tokio::test]
async fn list_writes_one_line_a_job() {
    let mut port = Port::new();
    port.listed = vec![
        listed(1, JobKind::Command, "跑全部测试", None, 72_134),
        listed(
            2,
            JobKind::Agent,
            "查 CI \"为什么\"红",
            Some("done"),
            125_000,
        ),
        listed(3, JobKind::Command, "build", Some("exited"), 41_000),
    ];
    let done = jobs(Some(port), json!({"action": "list"})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        concat!(
            "j1 command \"跑全部测试\": running, 72134 ms\n",
            "j2 agent \"查 CI \\u0022为什么\\u0022红\": done, 125000 ms\n",
            "j3 command \"build\": exited, 41000 ms\n",
        ),
        "标题里的引号照模板的规矩转义"
    );
    for port in [Some(Port::new()), None] {
        let done = jobs(port, json!({"action": "list"})).await;
        assert!(!done.error);
        assert_eq!(text(&done), "No jobs yet.\n", "一个都没有、没有端口的一样");
    }
}

#[tokio::test]
async fn output_pages_like_read() {
    let mut port = Port::new();
    port.command = Some("a\nb\nc".to_string());
    let done = jobs(Some(port), json!({"action": "output", "id": "j1"})).await;
    assert!(!done.error);
    assert_eq!(text(&done), "a\nb\nc\n", "最后一段没有换行的也算一行");
    let mut port = Port::new();
    port.command = Some("a\nb\nc\n".to_string());
    let done = jobs(
        Some(port),
        json!({"action": "output", "id": "j1", "offset": 2}),
    )
    .await;
    assert_eq!(text(&done), "b\nc\n");
    // 一页最多 30000 个字，停在整行上，带往下读的指路。
    let line = "字".repeat(10_000);
    let mut port = Port::new();
    port.command = Some(format!("{line}\n{line}\n{line}\n{line}\n"));
    let done = jobs(Some(port), json!({"action": "output", "id": "j1"})).await;
    assert_eq!(
        text(&done),
        format!("{line}\n{line}\n{line}\n(Showing lines 1-3 of 4. Use offset=4 to continue.)\n")
    );
    let mut port = Port::new();
    port.command = Some("a\nb\n".to_string());
    let done = jobs(
        Some(port),
        json!({"action": "output", "id": "j1", "offset": 3}),
    )
    .await;
    assert_eq!(
        text(&done),
        "(The output has 2 lines; offset 3 is past the end.)\n"
    );
}

#[tokio::test]
async fn output_says_what_is_still_going_on() {
    let mut port = Port::new();
    port.command = Some("building\n".to_string());
    port.running = true;
    let done = jobs(Some(port), json!({"action": "output", "id": "j1"})).await;
    assert_eq!(text(&done), "building\n(j1 is still running.)\n");
    for command in [None, Some(String::new())] {
        let mut port = Port::new();
        port.command = command;
        let done = jobs(Some(port), json!({"action": "output", "id": "j1"})).await;
        assert_eq!(text(&done), "No output.\n", "没存下来、空的一样");
    }
    // 子代理：最近的回答，这一步在跑的工具。
    let mut port = Port::new();
    port.reply = Some("Reading the tests first.".to_string());
    port.running = true;
    port.doing = vec!["read".to_string(), "grep".to_string()];
    let done = jobs(Some(port), json!({"action": "output", "id": "j2"})).await;
    assert_eq!(
        text(&done),
        "Reading the tests first.\n(j2 is still running. It is using read, grep now.)\n"
    );
    let mut port = Port::new();
    port.running = true;
    let done = jobs(Some(port), json!({"action": "output", "id": "j2"})).await;
    assert_eq!(text(&done), "No output.\n(j2 is still running.)\n");
}

#[tokio::test]
async fn stop_hands_the_job_to_the_port() {
    let port = Arc::new(Port::new());
    let site = Site::new();
    let done = site
        .done_jobs(
            "jobs",
            json!({"action": "stop", "id": "j1"}),
            Some(port.clone()),
            None,
        )
        .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Stopped j1.\n");
    let done = site
        .done_jobs(
            "jobs",
            json!({"action": "stop", "id": "j2"}),
            Some(port.clone()),
            None,
        )
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "j2 has already ended.\n");
    let done = site
        .done_jobs(
            "jobs",
            json!({"action": "stop", "id": "j9"}),
            Some(port.clone()),
            None,
        )
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "There is no job j9.\n");
    // 子会话派的带着前缀（施工 7-1 补）：几段的编号照样交给端口，不是 `j1`。
    let done = site
        .done_jobs(
            "jobs",
            json!({"action": "stop", "id": "j1.1"}),
            Some(port.clone()),
            None,
        )
        .await;
    assert_eq!(text(&done), "There is no job j1.1.\n");
    assert_eq!(
        port.stopped(),
        ["j1", "j2", "j9", "j1.1"].map(|id| JobId::parse(id).unwrap()),
        "照编号交给端口"
    );
}

#[tokio::test]
async fn ids_that_are_not_jobs_and_missing_ids() {
    for action in ["output", "stop"] {
        let done = jobs(Some(Port::new()), json!({"action": action, "id": "x\"1"})).await;
        assert!(done.error);
        assert_eq!(
            text(&done),
            "There is no job x\\u00221.\n",
            "不合编号写法的照没有，原样转义"
        );
        let done = jobs(None, json!({"action": action, "id": "j1"})).await;
        assert!(done.error);
        assert_eq!(text(&done), "There is no job j1.\n", "没有端口的照没有");
        let done = jobs(Some(Port::new()), json!({"action": action})).await;
        assert!(done.error);
        assert_eq!(
            text(&done),
            "The arguments are not right: missing field `id`.\n"
        );
    }
    let done = jobs(Some(Port::new()), json!({"action": "watch"})).await;
    assert!(done.error, "不认识的动作参数不对");
}

#[tokio::test]
async fn every_result_is_said_in_both_languages() {
    let mut checked = Vec::new();
    let mut port = Port::new();
    port.listed = vec![listed(1, JobKind::Command, "t", None, 1)];
    let done = jobs(Some(port), json!({"action": "list"})).await;
    check(
        &mut checked,
        human(done),
        said("jobs/listed/one").with("count", "1"),
    );
    let mut port = Port::new();
    port.listed = vec![
        listed(1, JobKind::Command, "a", None, 1),
        listed(2, JobKind::Command, "b", None, 1),
    ];
    let done = jobs(Some(port), json!({"action": "list"})).await;
    check(
        &mut checked,
        human(done),
        said("jobs/listed").with("count", "2"),
    );
    let done = jobs(Some(Port::new()), json!({"action": "list"})).await;
    check(&mut checked, human(done), said("jobs/none"));
    let done = jobs(Some(Port::new()), json!({"action": "output", "id": "j1"})).await;
    check(
        &mut checked,
        human(done),
        said("jobs/output").with("job", "j1"),
    );
    let done = jobs(Some(Port::new()), json!({"action": "stop", "id": "j1"})).await;
    check(
        &mut checked,
        human(done),
        said("jobs/stopped").with("job", "j1"),
    );
    let done = jobs(Some(Port::new()), json!({"action": "stop", "id": "j2"})).await;
    check(
        &mut checked,
        human(done),
        said("jobs/ended").with("job", "j2"),
    );
    let done = jobs(Some(Port::new()), json!({"action": "stop", "id": "j9"})).await;
    check(
        &mut checked,
        human(done),
        said("jobs/unknown").with("id", "j9"),
    );
    readable(&checked, &["jobs"]);
}
