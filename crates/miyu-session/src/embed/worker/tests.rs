//! 和 `miyu-embed` 说话的规矩（施工 R-5 中），对面是内存里的管道：`ready` 和报错的头一行；回的向量照收；回一句错的照收、
//! 还能接着问；回的编号对不上、读不懂、退出了、不回话的当它坏了。

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, duplex};

use super::{Conversation, Failure};

/// 一对管道：我们这一头的对话，和对面（假的小程序）的读、写两头。
fn pair() -> (
    Conversation<BufReader<DuplexStream>, DuplexStream>,
    BufReader<DuplexStream>,
    DuplexStream,
) {
    let (their_out, our_in) = duplex(1 << 16);
    let (our_out, their_in) = duplex(1 << 16);
    (
        Conversation::new(BufReader::new(our_in), our_out),
        BufReader::new(their_in),
        their_out,
    )
}

const SHORT: Duration = Duration::from_millis(200);

#[tokio::test]
async fn ready_reports_the_model_or_the_error() {
    let (mut talk, _, mut out) = pair();
    out.write_all(b"{\"ready\":{\"model\":\"local:tiny\",\"dims\":4}}\n")
        .await
        .unwrap();
    assert_eq!(talk.ready(SHORT).await, Ok("local:tiny".to_string()));

    let (mut talk, _, mut out) = pair();
    out.write_all(b"{\"error\":\"cannot load model.onnx\"}\n")
        .await
        .unwrap();
    assert_eq!(
        talk.ready(SHORT).await,
        Err("cannot load model.onnx".to_string())
    );

    let (mut talk, _, out) = pair();
    drop(out);
    assert_eq!(
        talk.ready(SHORT).await,
        Err("miyu-embed exited".to_string())
    );

    let (mut talk, _keep_in, _keep_out) = pair();
    assert_eq!(
        talk.ready(SHORT).await,
        Err("not ready after 0.2 seconds".to_string())
    );
}

#[tokio::test]
async fn answers_are_matched_by_id() {
    let (mut talk, mut input, mut out) = pair();
    let peer = tokio::spawn(async move {
        let mut line = String::new();
        input.read_line(&mut line).await.unwrap();
        assert_eq!(line, "{\"id\":\"1\",\"text\":\"猫\"}\n");
        out.write_all(b"{\"id\":\"1\",\"vector\":[0.6,0.8]}\n")
            .await
            .unwrap();
        line.clear();
        input.read_line(&mut line).await.unwrap();
        out.write_all(b"{\"id\":\"2\",\"error\":\"the vector is zero or not a number\"}\n")
            .await
            .unwrap();
        line.clear();
        input.read_line(&mut line).await.unwrap();
        out.write_all(b"{\"id\":\"3\",\"vector\":[1.0]}\n")
            .await
            .unwrap();
        (input, out)
    });
    assert_eq!(talk.ask("猫", SHORT).await, Ok(vec![0.6, 0.8]));
    assert_eq!(
        talk.ask("", SHORT).await,
        Err(Failure::Refused(
            "the vector is zero or not a number".to_string()
        )),
        "它回一句错：这一条算不出"
    );
    assert_eq!(talk.ask("狗", SHORT).await, Ok(vec![1.0]), "回过错的接着问");
    peer.await.unwrap();
}

#[tokio::test]
async fn a_wrong_id_garbage_an_exit_or_silence_means_it_is_broken() {
    let broken = |failure: Result<Vec<f32>, Failure>, said: &str| match failure {
        Err(Failure::Broken(why)) => assert!(why.contains(said), "{why}"),
        other => panic!("该当它坏了：{other:?}"),
    };
    let (mut talk, _input, mut out) = pair();
    out.write_all(b"{\"id\":\"9\",\"vector\":[1.0]}\n")
        .await
        .unwrap();
    broken(talk.ask("猫", SHORT).await, "another request");

    let (mut talk, _input, mut out) = pair();
    out.write_all(b"not json\n").await.unwrap();
    broken(talk.ask("猫", SHORT).await, "unreadable");

    let (mut talk, _input, mut out) = pair();
    out.write_all(b"{\"id\":\"1\",\"vector\":[\"x\"]}\n")
        .await
        .unwrap();
    broken(talk.ask("猫", SHORT).await, "no vector");

    let (mut talk, _input, out) = pair();
    drop(out);
    broken(talk.ask("猫", SHORT).await, "exited");

    let (mut talk, _input, _out) = pair();
    broken(talk.ask("猫", SHORT).await, "no answer after 0.2 seconds");
}
