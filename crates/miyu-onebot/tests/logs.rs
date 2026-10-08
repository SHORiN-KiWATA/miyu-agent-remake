//! `miyu onebot logs`（施工 O-18，`onebot.md` 第一条「对外的样子」、「施工时定的」第 26 条）：只有运行日志的原样印；标准错误
//! 那一份有内容的先印它，两段各带一行标题；还没有运行日志的在标准错误上说一句；`-f` 接着印新写的，文件换了从头读。

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use miyu_onebot::logs::{Heading, logs};
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::spawning::program;
use crate::support::*;

/// 运行日志、标准错误那一份在哪。
fn files(root: &DataRoot) -> (PathBuf, PathBuf) {
    let dir = root.state().join("logs");
    std::fs::create_dir_all(&dir).expect("建得了");
    (dir.join("onebot.log"), dir.join("onebot.stderr"))
}

/// 印一次（不跟着看）：交回退出码、标准输出、标准错误。
fn shown(root: &DataRoot) -> (u8, String, String) {
    let texts = Texts::load(ResourceRoot::at(resources()), "zh").expect("读得出来");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = logs(root, None, &texts, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("UTF-8"),
        String::from_utf8(err).expect("UTF-8"),
    )
}

fn zh(heading: &Heading) -> String {
    Texts::load(ResourceRoot::at(resources()), "zh")
        .expect("读得出来")
        .heading(heading)
}

#[test]
fn the_run_log_alone_is_printed_as_it_is() {
    let (dir, root) = temp_root();
    let (log, stderr) = files(&root);
    std::fs::write(&log, "2026-10-08 INFO a\n2026-10-08 INFO b\n").expect("写得进");
    assert_eq!(
        shown(&root),
        (
            0,
            "2026-10-08 INFO a\n2026-10-08 INFO b\n".to_string(),
            String::new()
        )
    );
    std::fs::write(&stderr, "").expect("写得进");
    assert_eq!(
        shown(&root).1,
        "2026-10-08 INFO a\n2026-10-08 INFO b\n",
        "空的标准错误不另起一段"
    );
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[test]
fn stderr_with_something_in_it_comes_first_under_its_own_heading() {
    let (dir, root) = temp_root();
    let (log, stderr) = files(&root);
    std::fs::write(&log, "INFO a\n").expect("写得进");
    std::fs::write(&stderr, "端口 8301 被占了。").expect("写得进");
    let expected = format!(
        "{}\n端口 8301 被占了。\n{}\nINFO a\n",
        zh(&Heading::Stderr(stderr.display().to_string())),
        zh(&Heading::Log(log.display().to_string())),
    );
    assert_eq!(
        shown(&root),
        (0, expected, String::new()),
        "没换行结尾的补上"
    );
    std::fs::remove_file(&log).expect("删得掉");
    let (code, out, err) = shown(&root);
    assert_eq!(code, 0);
    assert!(out.ends_with(&format!(
        "{}\n",
        zh(&Heading::Log(log.display().to_string()))
    )));
    assert_eq!(
        err,
        format!("{}\n", zh(&Heading::None(log.display().to_string())))
    );
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[test]
fn no_log_yet_is_said_on_stderr() {
    let (dir, root) = temp_root();
    let (log, _) = files(&root);
    assert_eq!(
        shown(&root),
        (
            0,
            String::new(),
            format!("{}\n", zh(&Heading::None(log.display().to_string())))
        )
    );
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn follow_prints_what_is_written_later_and_starts_over_on_a_new_file() {
    let (dir, root) = temp_root();
    let (log, _) = files(&root);
    std::fs::write(&log, "first line that is fairly long\n").expect("写得进");
    let mut command = tokio::process::Command::from(program(&root, &["logs", "-f"]));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().expect("起得来");
    let mut lines = BufReader::new(child.stdout.take().expect("接了管道")).lines();
    let mut next = async || {
        within("印出下一行", lines.next_line())
            .await
            .expect("读得了")
            .expect("还在跟着")
    };
    assert_eq!(next().await, "first line that is fairly long");
    let mut appended = std::fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .expect("开得了");
    std::io::Write::write_all(&mut appended, b"second\n").expect("写得进");
    drop(appended);
    assert_eq!(next().await, "second");
    // 换了一份（比原来短）：从头读。
    std::fs::rename(&log, log.with_extension("log.old")).expect("改得了名");
    std::fs::write(&log, "fresh\n").expect("写得进");
    assert_eq!(next().await, "fresh");
    child.kill().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}
