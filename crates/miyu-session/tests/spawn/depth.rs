//! 派子代理的深度上限、子代理自己派的编号（施工 7-5、7-1 补，从 `spawn.rs` 拆出来守 500 行上限，施工 C-4 加`read_log`
//! 顶到了行数上限）：本机没到深度上限能派、到了上限和场所会话（群）不给 `subagent`；子代理领的编号带前缀，载入以后
//! 接着日志往下数。

use super::*;

/// 父会话 `depth` 层的子会话。
fn child_at(depth: u32) -> Lines {
    Lines {
        lineage: Some(Lineage {
            parent: child_id(9),
            depth,
        }),
        ..Lines::default()
    }
}

#[tokio::test]
async fn only_local_sessions_below_the_depth_limit_can_spawn() {
    let persona = "You are a helpful software engineer.";
    let lines = LINES.trim_end();
    // 主会话：有 `subagent`、没有以前的名字 `agent`（施工 7-5 再补），system 是人设和核心的几行（施工 2-7 补）。
    let table = Arc::new(Table::default());
    let (request, _) = first_request(Lines::default(), &table).await;
    assert!(names(&request).contains(&"subagent"));
    assert!(!names(&request).contains(&"agent"), "{:?}", names(&request));
    assert_eq!(request.system, format!("{persona}\n\n{lines}"));
    // 第 1 层：还能派孙代理；场所说明接在人设后面，核心的几行在最后。
    let (request, _) = first_request(child_at(1), &table).await;
    assert!(names(&request).contains(&"subagent"));
    assert_eq!(
        request.system,
        format!("{persona}\n\n{}\n\n{lines}", VENUE.trim_end())
    );
    assert_eq!(table.made().len(), 2);
    assert_eq!(table.made()[1].lineage.depth, 2, "孙代理是第 2 层");

    // 第 2 层到了上限、场所会话（群）：工具面里没有 `subagent`，调了只会被当成没有的工具拒掉，一个都派不出去。
    let group = Lines {
        venue: VenueId::parse("qq:group:123456").unwrap(),
        ..Lines::default()
    };
    for (lines, child) in [(child_at(2), true), (group, false)] {
        let table = Arc::new(Table::default());
        let (request, results) = first_request(lines, &table).await;
        assert!(
            !names(&request).contains(&"subagent"),
            "{:?}",
            names(&request)
        );
        assert!(names(&request).contains(&"read"), "别的工具照给");
        assert_eq!(request.system.contains(VENUE.trim_end()), child);
        let [result] = results.try_into().expect("一次调用");
        assert_eq!(text(&result), "There is no tool named \"subagent\".\n");
        assert!(table.made().is_empty());
    }
}

/// 载入的子会话照 `session.created` 记得自己是第几层（施工 7-5）：第 1 层载入以后再派，派的是第 2 层。
#[tokio::test]
async fn a_loaded_child_still_knows_its_depth() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("甲", "Task A.")]),
        Play::Says("好。"),
        calls(&[subagent("乙", "Task B.")]),
        Play::Says("好。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        ..child_at(1)
    };
    let tools = basesystem(&home);
    let handle = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    one_turn(&home, &handle, 1).await;
    let id = handle.id().clone();
    stop(&handle).await;
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let cwd = environment().cwd;
    let loaded = home.load_full(&id, &script, &tools, &cwd, port).await;
    one_turn(&home, &loaded, 2).await;
    let depths: Vec<(SessionId, u32)> = table
        .made()
        .into_iter()
        .map(|child| (child.lineage.parent, child.lineage.depth))
        .collect();
    assert_eq!(depths, [(id.clone(), 2), (id, 2)]);
}

/// 子会话派的编号带上它自己在父会话里的编号（施工 7-1 补，`agents.md`「对外的样子」）：造它的命令是 `<父会话>/j2`，它派的是
/// `j2.1`，交给会话表的命令编号、结果那一句、效果都是；载入以后照 `session.created` 的 `cause` 读回前缀，接着是 `j2.2`。
#[tokio::test]
async fn a_child_numbers_its_jobs_under_its_own() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("甲", "Task A.")]),
        Play::Says("好。"),
        calls(&[subagent("乙", "Task B.")]),
        Play::Says("好。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        command: Some(CommandId::parse(&format!("{}/j2", child_id(9))).unwrap()),
        ..child_at(1)
    };
    let tools = basesystem(&home);
    let handle = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    let log = one_turn(&home, &handle, 1).await;
    let id = handle.id().clone();
    let [result] = results(&log).try_into().expect("一次调用");
    assert_eq!(text(result), "Started subagent j2.1: \"甲\".\n");
    assert_eq!(
        result.effects,
        [Effect::JobStarted(JobStarted {
            job: JobId::parse("j2.1").unwrap(),
            what: JobKind::Agent,
            title: "甲".to_string(),
            session: Some(child_id(1)),
        })]
    );
    let [(_, prompt, _, _)] = table.sent().try_into().expect("送了一次交代");
    assert_eq!(prompt.as_str(), format!("{id}/j2.1/prompt"));

    stop(&handle).await;
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let cwd = environment().cwd;
    let loaded = home.load_full(&id, &script, &tools, &cwd, port).await;
    let log = one_turn(&home, &loaded, 2).await;
    assert_eq!(
        text(results(&log)[1]),
        "Started subagent j2.2: \"乙\".\n",
        "载入以后照造它的命令读回前缀，接着往下数"
    );
    let commands: Vec<String> = table
        .made()
        .iter()
        .map(|child| child.command.as_str().to_string())
        .collect();
    assert_eq!(commands, [format!("{id}/j2.1"), format!("{id}/j2.2")]);
}
