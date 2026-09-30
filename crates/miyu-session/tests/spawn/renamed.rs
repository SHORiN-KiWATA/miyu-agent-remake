//! 派子代理的那件从 `agent` 改名 `subagent`（施工 7-5 再补，`docs/blueprint/tools/subagent.md`「以前的名字」）：新造的会话
//! 调 `agent` 照没有的工具拒掉；改名以前造的会话，快照里冻着 `agent`，换了新的核心载入以后工具面一个字节不变，调 `agent`
//! 照样派得出子代理。两张工具面只差那件的名字，和名字带来的先后。

use miyu_kernel::request::ToolSpec;
use miyu_tool::Tool;
use miyu_tool::testkit::Renamed;

use super::*;

/// 改名以前的核心的目录：派子代理的那件叫 `agent`，说明、参数格式、访问类别和现在的一样（改名没动它们），别的几件照出厂的。
fn before_the_rename(home: &Home) -> Catalog {
    let tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    assert!(
        tools.iter().any(|tool| tool.spec().name == "subagent"),
        "出厂的目录里有 subagent"
    );
    let tools = tools
        .into_iter()
        .map(|tool| match tool.spec().name.as_str() {
            "subagent" => Renamed::new(tool, "agent") as Arc<dyn Tool>,
            _ => tool,
        });
    Catalog::new(tools).expect("合写法")
}

/// 照以前的名字调一次。
fn agent(title: &str) -> (&'static str, String) {
    let args = serde_json::json!({"description": title, "prompt": "Task."});
    ("agent", args.to_string())
}

/// 工具面照请求里的样子写成字节：名字、说明、参数格式照先后。
fn face(request: &Request) -> String {
    serde_json::to_string(&request.tools).expect("写得成 JSON")
}

#[tokio::test]
async fn a_new_session_refuses_the_old_name() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([calls(&[agent("甲")]), Play::Says("好。")]);
    let handle = parent(&home, &script, &table).await;
    let log = one_turn(&home, &handle, 1).await;
    let [result] = results(&log).try_into().expect("一次调用");
    assert_eq!(text(result), "There is no tool named \"agent\".\n");
    assert!(table.made().is_empty(), "一个都没派");
}

#[tokio::test]
async fn a_session_made_before_the_rename_keeps_its_face_and_agent_still_spawns() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[agent("甲")]),
        Play::Says("好。"),
        calls(&[agent("乙")]),
        Play::Says("好。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let old = home
        .create_full(
            &script,
            &before_the_rename(&home),
            Opening::default(),
            lines,
        )
        .await;
    one_turn(&home, &old, 1).await;
    let id = old.id().clone();
    stop(&old).await;

    // 换成现在的核心载入：工具面照快照发，`agent` 照样派得出去。
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let cwd = environment().cwd;
    let loaded = home
        .load_full(&id, &script, &basesystem(&home), &cwd, port)
        .await;
    let log = one_turn(&home, &loaded, 2).await;
    assert_eq!(text(results(&log)[1]), "Started subagent j2: \"乙\".\n");
    assert_eq!(table.made().len(), 2, "两次都派出去了");
    let [(_, before), _, (_, after), _] = script.requests().try_into().expect("两轮四次请求");
    assert_eq!(face(&after), face(&before), "工具面一个字节不变");
    assert!(names(&after).contains(&"agent"));
    assert!(!names(&after).contains(&"subagent"), "{:?}", names(&after));

    // 和新造的会话比：只差那件的名字，和照名字排带来的先后。
    let (new, _) = first_request(Lines::default(), &Arc::new(Table::default())).await;
    let mut renamed: Vec<ToolSpec> = before.tools.clone();
    for tool in &mut renamed {
        if tool.name == "agent" {
            tool.name = "subagent".to_string();
        }
    }
    renamed.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(serde_json::to_string(&renamed).unwrap(), face(&new));
    assert_ne!(face(&new), face(&before));
}
