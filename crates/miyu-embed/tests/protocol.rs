//! 小程序的协议（`recall.md` 第四条第 4 款），手造的小模型（`fixtures/tiny/`）：起来先说 `ready`；一行一句，回一行向量，
//! 和 Python 的 ONNX Runtime 算的逐个比；坏的一行回错、接着读；超长的截断；标准输入关了退出码 0；清单、文件不对的说一行
//! 错、退出码 1；参数写错退出码 2。

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::support::{fixtures, read};

/// 小模型对一组编号的输出（`fixtures/tiny/expected.json`）。
#[derive(Deserialize)]
struct Expected {
    ids: Vec<i64>,
    vector: Vec<f32>,
}

/// 一个跑着的小程序。
struct Embed {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Embed {
    /// 照 `args` 起一个。
    fn spawn(args: &[&std::ffi::OsStr]) -> Embed {
        let mut child = Command::new(env!("CARGO_BIN_EXE_miyu-embed"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("起得来");
        let input = child.stdin.take().expect("有标准输入");
        let output = BufReader::new(child.stdout.take().expect("有标准输出"));
        Embed {
            child,
            input,
            output,
        }
    }

    /// 照小模型的清单起一个。
    fn tiny() -> Embed {
        let dir = fixtures().join("tiny");
        Embed::spawn(&[
            "--manifest".as_ref(),
            dir.join("manifest.toml").as_os_str(),
            "--dir".as_ref(),
            dir.as_os_str(),
        ])
    }

    /// 读一行回应。
    fn line(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).expect("读得了");
        serde_json::from_str(&line).unwrap_or_else(|error| panic!("{line:?} 不是 JSON：{error}"))
    }

    /// 送一行，读一行。
    fn ask(&mut self, line: &str) -> Value {
        writeln!(self.input, "{line}").expect("送得进");
        self.line()
    }

    /// 关掉标准输入，等它退出，交回退出码。
    fn close(self) -> Option<i32> {
        let Embed {
            mut child, input, ..
        } = self;
        drop(input);
        child.wait().expect("等得到").code()
    }
}

/// 一行请求。
fn request(id: &str, text: &str) -> String {
    json!({"id": id, "text": text}).to_string()
}

/// 回应里的向量。
fn vector(reply: &Value) -> Vec<f32> {
    let values = reply["vector"]
        .as_array()
        .unwrap_or_else(|| panic!("没有向量：{reply}"));
    values
        .iter()
        .map(|value| value.as_f64().expect("是数") as f32)
        .collect()
}

/// 两个向量差不多一样：每一格差不过 1e-6（Python 那边是另一个版本的 ONNX Runtime，加法的先后也可能不同）。
fn same(got: &[f32], want: &[f32]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a - b).abs() <= 1e-6)
}

fn expected() -> Vec<Expected> {
    serde_json::from_str(&read("tiny/expected.json")).expect("合写法")
}

#[test]
fn it_says_ready_then_answers_each_line() {
    let mut embed = Embed::tiny();
    assert_eq!(
        embed.line(),
        json!({"ready": {"model": "local:tiny", "dims": 4}})
    );
    let expected = expected();
    // 小词表上这几句切成的编号正是 expected.json 里的前四组：我的猫、喝茶、hello world!、不认识的字。
    for (k, text) in ["我的猫", "喝茶", "hello world!", "鱼"].iter().enumerate() {
        let reply = embed.ask(&request(&format!("r{k}"), text));
        assert_eq!(reply["id"], json!(format!("r{k}")));
        let got = vector(&reply);
        assert!(
            same(&got, &expected[k].vector),
            "{text}：{got:?} 对 {:?}",
            expected[k].vector
        );
    }
    assert_eq!(embed.close(), Some(0), "标准输入关了就退出");
}

#[test]
fn a_long_text_is_cut_and_an_empty_one_still_has_a_vector() {
    let mut embed = Embed::tiny();
    embed.line();
    let expected = expected();
    assert_eq!(expected[4].ids, [2, 5, 5, 5, 5, 3]);
    let reply = embed.ask(&request("long", &"猫".repeat(30)));
    assert!(
        same(&vector(&reply), &expected[4].vector),
        "照清单的 max_tokens 截，留住 [SEP]"
    );
    assert_eq!(expected[5].ids, [2, 3]);
    let reply = embed.ask(&request("empty", ""));
    assert!(
        same(&vector(&reply), &expected[5].vector),
        "空的是 [CLS][SEP]"
    );
    embed.close();
}

#[test]
fn a_bad_line_gets_an_error_and_the_next_one_is_answered() {
    let mut embed = Embed::tiny();
    embed.line();
    let reply = embed.ask("not json");
    assert_eq!(reply["id"], Value::Null, "{reply}");
    assert!(
        reply["error"]
            .as_str()
            .is_some_and(|said| said.starts_with("bad request")),
        "{reply}"
    );
    let reply = embed.ask(r#"{"id":"r1"}"#);
    assert_eq!(reply, json!({"id": "r1", "error": "bad request: no text"}));
    let reply = embed.ask(r#"{"id":7,"text":"猫"}"#);
    assert_eq!(reply, json!({"id": null, "error": "bad request: no id"}));
    let reply = embed.ask(r#"{"text":"猫"}"#);
    assert_eq!(reply, json!({"id": null, "error": "bad request: no id"}));
    let reply = embed.ask("");
    assert_eq!(reply["id"], Value::Null, "空行也回：{reply}");
    let reply = embed.ask(&request("r2", "喝茶"));
    assert!(same(&vector(&reply), &expected()[1].vector), "接着读下一行");
    // 不是 UTF-8 的一行。
    embed.input.write_all(b"\xff\xfe\n").expect("送得进");
    let reply = embed.line();
    assert_eq!(
        reply,
        json!({"id": null, "error": "bad request: not UTF-8"})
    );
    let reply = embed.ask(&format!("{}\r", request("r3", "喝茶")));
    assert_eq!(
        reply["id"],
        json!("r3"),
        "行尾的 \\r 是 JSON 的空白，不算：{reply}"
    );
    assert_eq!(embed.close(), Some(0));
}

/// 起不来的：标准输出一行错、退出码 1。
fn refused(manifest: &Path, dir: &Path) -> (Value, Option<i32>) {
    let mut embed = Embed::spawn(&[
        "--manifest".as_ref(),
        manifest.as_os_str(),
        "--dir".as_ref(),
        dir.as_os_str(),
    ]);
    let reply = embed.line();
    (reply, embed.close())
}

#[test]
fn a_bad_manifest_or_missing_files_exit_with_one() {
    let tiny = fixtures().join("tiny");
    let (reply, code) = refused(&tiny.join("nothing.toml"), &tiny);
    assert!(
        reply["error"]
            .as_str()
            .is_some_and(|said| said.starts_with("cannot read")),
        "{reply}"
    );
    assert_eq!(code, Some(1));
    let (reply, code) = refused(&tiny.join("manifest.toml"), &fixtures());
    assert!(
        reply["error"]
            .as_str()
            .is_some_and(|said| said.contains("model.onnx")),
        "文件不在：{reply}"
    );
    assert_eq!(code, Some(1));
    // 模型文件是坏的：照清单找得到，载入不了。
    let (reply, code) = refused(
        &fixtures().join("tiny/manifest.toml"),
        &fixtures().join("broken"),
    );
    assert!(
        reply["error"]
            .as_str()
            .is_some_and(|said| said.starts_with("cannot load")),
        "{reply}"
    );
    assert_eq!(code, Some(1));
}

#[test]
fn a_manifest_with_the_wrong_dims_answers_each_line_with_an_error() {
    let tiny = fixtures().join("tiny");
    let mut embed = Embed::spawn(&[
        "--manifest".as_ref(),
        tiny.join("wrong-dims.toml").as_os_str(),
        "--dir".as_ref(),
        tiny.as_os_str(),
    ]);
    assert_eq!(
        embed.line(),
        json!({"ready": {"model": "local:tiny", "dims": 5}})
    );
    let reply = embed.ask(&request("r1", "猫"));
    assert_eq!(
        reply,
        json!({"id": "r1", "error": "the model gives 4 dimensions, the manifest says 5"})
    );
    assert_eq!(embed.close(), Some(0));
}

#[test]
fn wrong_arguments_exit_with_two() {
    for args in [
        vec![],
        vec!["--manifest"],
        vec!["--dir", "x"],
        vec!["--manifest", "a", "--dir", "b", "--extra"],
        vec!["--manifest", "a", "--manifest", "b", "--dir", "c"],
    ] {
        let args: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_ref()).collect();
        let output = Command::new(env!("CARGO_BIN_EXE_miyu-embed"))
            .args(&args)
            .stdin(Stdio::null())
            .output()
            .expect("起得来");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "标准输出不写：{args:?}");
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(
            said.contains("usage: miyu-embed --manifest <file> --dir <directory>"),
            "{said}"
        );
    }
}
