//! 探针的存档（施工 7-10 从 `probe.rs` 挪出来，`probe_harness.rs` 也用）：存档在 `docs/designs/samples/probe/<会话>/`，
//! `log.jsonl` 是真内核记下的日志，`requests/` 下一次请求一个文件（规范字节，末尾一个换行），`openai-chat/` 下是同一次请求
//! 编码成 OpenAI 兼容接口的字节。字节变了必须是有意的：设上 `MIYU_PROBE_WRITE=1` 跑一遍，重写存档，提交说明里写为什么变。

use std::fs;
use std::path::PathBuf;

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
    for (index, (_, request)) in stage.requests().iter().enumerate() {
        let bytes = String::from_utf8(request.canonical_bytes()).expect("规范的字节是 UTF-8");
        files.push((format!("requests/{:02}.json", index + 1), bytes + "\n"));
        let body = String::from_utf8(wire(request).body).expect("请求字节是 UTF-8");
        files.push((format!("openai-chat/{:02}.json", index + 1), body + "\n"));
    }
    files
}

/// 这段会话和存档 `name` 逐字节比；设了 `MIYU_PROBE_WRITE` 的重写存档。
pub fn matches_the_archive(name: &str, stage: &Stage) {
    let files = files(stage);
    let dir = archive(name);
    if std::env::var_os("MIYU_PROBE_WRITE").is_some() {
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("删得掉旧的存档");
        }
        fs::create_dir_all(dir.join("requests")).expect("建得了存档目录");
        fs::create_dir_all(dir.join("openai-chat")).expect("建得了存档目录");
        for (name, content) in &files {
            fs::write(dir.join(name), content).expect("写得了存档");
        }
        return;
    }
    for (name, content) in &files {
        let path = dir.join(name);
        let archived =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        assert!(
            archived == *content,
            "{name} 和存档不一样。要是有意改的，设上 MIYU_PROBE_WRITE=1 跑一遍重写存档，提交说明里写为什么变"
        );
    }
    for folder in ["requests", "openai-chat"] {
        let archived = fs::read_dir(dir.join(folder))
            .expect("读得了存档的请求目录")
            .count();
        assert_eq!(
            archived,
            (files.len() - 1) / 2,
            "存档 {folder}/ 里的请求数和这一次的不一样"
        );
    }
}
