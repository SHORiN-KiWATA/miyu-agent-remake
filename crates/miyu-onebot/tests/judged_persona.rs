//! 判官带人格（施工 O-23 补，`onebot.md` 第一条「群里怎么叫她」第 1 条、第 12 条第 3 款）：真核心照开关拉起真桥，系统区装上
//! 样本人格，判官那一次发到本机回环上的假服务器（同 `judged.rs`）。设了人格的群，判官的 system 里 `system.txt` 后面紧跟着人格的
//! 两个标签夹着人设；关了的群、无人格的群不带；会话造好以后人格删了的不带、照样问、记一行运行日志。

use serde_json::json;

use miyu_http::testkit::Server;
use miyu_session::testkit::{
    SAMPLE_PERSONA, Script, install_sample_persona, sample_persona_dir, sample_persona_texts,
};

use crate::judged::{JIE, LIN, MEMBERS, decided, judge_text, until_decided};
use crate::support::group::*;
use crate::support::judge::*;
use crate::support::*;

/// 用样本人格的群（施工 O-23 补）。
const PERSONA: i64 = 560;

/// 用样本人格、判官不带人格的群。
const PERSONA_OFF: i64 = 561;

/// 用一个后来删掉的人格的群。
const DOOMED: i64 = 562;

/// 后来删掉的那个人格的编号：样本照抄一份。
const DOOMED_PERSONA: &str = "doomed";

/// 无人格的群。
const PLAIN: i64 = 563;

/// 判官带人格的几条规则：[`PLAIN`] 无人格，[`PERSONA`]、[`PERSONA_OFF`] 用样本人格（后一个关掉判官的人格），[`DOOMED`] 用
/// [`DOOMED_PERSONA`]；群默认不抽样。
fn persona_rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\n\n\
         [[rule]]\nmatch = {{ group = [{PERSONA}, {PERSONA_OFF}] }}\npersona = \"{SAMPLE_PERSONA}\"\n\n\
         [[rule]]\nmatch = {{ group = [{PERSONA_OFF}] }}\njudge = {{ persona = false }}\n\n\
         [[rule]]\nmatch = {{ group = [{DOOMED}] }}\npersona = \"{DOOMED_PERSONA}\"\n"
    )
}

/// 样本人格照抄一份进系统区那一层，编号是 `id`。
fn install_copy(home: &Home, id: &str) {
    let to = home.root.system().join("personas").join(id);
    std::fs::create_dir_all(to.join("prompts")).expect("建得了目录");
    for file in ["persona.toml", "prompts/persona.md"] {
        std::fs::copy(sample_persona_dir().join(file), to.join(file)).expect("复制得了");
    }
}

/// 判官收到的第 `n` 个请求的 system。
fn system_asked(server: &Server, n: usize) -> String {
    asked(server, n)["messages"][0]["content"]
        .as_str()
        .expect("是字")
        .to_string()
}

#[tokio::test]
async fn the_judge_brings_the_persona_of_the_group() {
    let server = judge(vec![
        no("with persona"),
        no("persona off"),
        no("no persona"),
        no("persona gone"),
    ])
    .await;
    let script = Script::new([]);
    let (home, mut napcat, _) =
        started_judged(&script, &server, (&persona_rules(), ""), "", MEMBERS).await;
    install_sample_persona(&home.root);
    install_copy(&home, DOOMED_PERSONA);
    // 1：用样本人格的群，2：同样的人格、判官不带，3：无人格的群；都是小林 @ 她，判官说不回。
    for (group, message) in [(PERSONA, 1), (PERSONA_OFF, 2), (PLAIN, 3)] {
        napcat.send(group_frame(
            group,
            LIN,
            message,
            json!([at(BOT), plain(" 在吗")]),
            ("小林", "lin"),
        ));
        until_decided(&home, group, message).await;
    }
    // 4：阿杰随口一句，只记下，群会话造好了；人格删了以后，5：小林 @ 她，判官照样问、不带人格。
    napcat.send(group_frame(
        DOOMED,
        JIE,
        4,
        json!([plain("路过")]),
        ("阿杰", "jie"),
    ));
    until_decided(&home, DOOMED, 4).await;
    std::fs::remove_dir_all(home.root.system().join("personas").join(DOOMED_PERSONA))
        .expect("删得了");
    napcat.send(group_frame(
        DOOMED,
        LIN,
        5,
        json!([at(BOT), plain(" 在吗")]),
        ("小林", "lin"),
    ));
    let doomed = until_decided(&home, DOOMED, 5).await;
    assert_eq!(
        decided(&doomed, 5).expect("记了判断")["judge"]["answer"]["reason"],
        "persona gone",
        "照样问了"
    );

    // 带人格的：system.txt 后面紧跟着人格的两个标签夹着样本的人设，再是打分的说明。
    let persona = format!(
        "{}{}{}{}{}",
        judge_text("system.txt"),
        judge_text("persona-open.txt"),
        sample_persona_texts().persona,
        judge_text("persona-close.txt"),
        judge_text("reply.txt"),
    );
    let with = system_asked(&server, 0);
    assert!(with.starts_with(&persona), "{with}");
    let open = judge_text("persona-open.txt");
    for (n, why) in [
        (1, "关了的群不带"),
        (2, "无人格的群不带"),
        (3, "人格删了的不带"),
    ] {
        let system = system_asked(&server, n);
        assert!(!system.contains(&open), "{why}：{system}");
        assert!(
            system.starts_with(&format!(
                "{}{}",
                judge_text("system.txt"),
                judge_text("reply.txt")
            )),
            "{why}：{system}"
        );
    }
    let log = run_log(&home.root);
    assert!(log.contains("persona not read"), "{log}");
    assert!(log.contains(DOOMED_PERSONA), "{log}");
    assert_eq!(script.requests().len(), 0, "判官都说不回");
    assert!(napcat.pending().is_none(), "别的什么都不发");
    stopped(home).await;
}
