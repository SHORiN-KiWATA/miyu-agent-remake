//! 探针的存档（施工 7-10 从 `probe.rs` 挪出来，`probe_harness.rs` 也用）：存档在 `docs/designs/samples/probe/<会话>/`，
//! `log.jsonl` 是真内核记下的日志，`requests/` 下一次请求一个文件（规范字节，末尾一个换行），`openai-chat/` 下是同一次请求
//! 编码成 OpenAI 兼容接口的字节。要过回顾的（施工 3-8 四补），回顾的请求另放在 `recaps/`、`recaps/openai-chat/` 下：它是
//! 单独的一次请求，照它自己的存档比。字节变了必须是有意的：设上 `MIYU_PROBE_WRITE=1` 跑一遍，重写存档，提交说明里写为什么变。

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use miyu_kernel::id::Seq;
use miyu_kernel::request::Request;
use miyu_kernel::testkit::Stage;

use super::{lines, wire};

/// 存档所在的目录：这个 crate 的目录往上两级是仓库根。
fn archive(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/probe")
        .join(name)
}

/// 这段会话要存档的几个文件：相对存档目录的路径，和内容。
pub fn files(stage: &Stage) -> Vec<(String, String)> {
    let mut files = vec![("log.jsonl".to_string(), lines(stage).join("\n") + "\n")];
    requests(&mut files, "", stage.requests());
    requests(&mut files, "recaps/", stage.recaps());
    files
}

/// 一次请求两个文件，放在 `folder` 下：`requests/` 里是规范字节，`openai-chat/` 里是编码成 OpenAI 兼容接口的字节。
fn requests(files: &mut Vec<(String, String)>, folder: &str, requests: &[(Seq, Request)]) {
    for (index, (_, request)) in requests.iter().enumerate() {
        let bytes = String::from_utf8(request.canonical_bytes()).expect("规范的字节是 UTF-8");
        let name = |kind: &str| match folder {
            "" => format!("{kind}/{:02}.json", index + 1),
            _ if kind == "requests" => format!("{folder}{:02}.json", index + 1),
            _ => format!("{folder}{kind}/{:02}.json", index + 1),
        };
        files.push((name("requests"), bytes + "\n"));
        let body = String::from_utf8(wire(request).body).expect("请求字节是 UTF-8");
        files.push((name("openai-chat"), body + "\n"));
    }
}

/// 这段会话和存档 `name` 逐字节比；设了 `MIYU_PROBE_WRITE` 的重写存档。
pub fn matches_the_archive(name: &str, stage: &Stage) {
    let files = files(stage);
    let dir = archive(name);
    if std::env::var_os("MIYU_PROBE_WRITE").is_some() {
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("删得掉旧的存档");
        }
        for (name, content) in &files {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().expect("存档里的文件有目录")).expect("建得了存档目录");
            fs::write(path, content).expect("写得了存档");
        }
        return;
    }
    let mut counts: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for (name, content) in &files {
        let path = dir.join(name);
        let archived =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        assert!(
            archived == *content,
            "{name} 和存档不一样。要是有意改的，设上 MIYU_PROBE_WRITE=1 跑一遍重写存档，提交说明里写为什么变"
        );
        *counts
            .entry(path.parent().expect("存档里的文件有目录").to_path_buf())
            .or_default() += 1;
    }
    for (folder, count) in counts.into_iter().filter(|(folder, _)| *folder != dir) {
        let archived = fs::read_dir(&folder)
            .expect("读得了存档的请求目录")
            .filter(|entry| entry.as_ref().is_ok_and(|entry| entry.path().is_file()))
            .count();
        assert_eq!(
            archived,
            count,
            "存档 {} 里的请求数和这一次的不一样",
            folder.display()
        );
    }
}
