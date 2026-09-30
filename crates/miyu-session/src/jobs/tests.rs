//! 任务表的测试（施工 7-3）：用假的后台进程（`miyu_tool::testkit::Held`），三个平台一样。编号照日志往后数；输出一段段
//! 写进 `jobs/<编号>.out`；自己退出了整份存成 blob、交回 actor，`by` 是起它的那次调用，`cause` 是那一轮的；交进内核、
//! 落了盘才从表里拿掉；停下时在跑的报 `restarted`、已经报了的不再报，杀掉的是还在跑的；actor 停了整组杀掉、不再收新的。

use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use miyu_kernel::id::ContentHash;
use miyu_tool::Exit;
use miyu_tool::testkit::Held;

use super::*;

/// 一个用完就删的临时目录：会话目录是 `session/`，blob 在 `blobs/`。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-jobs-{}-{n}", std::process::id()));
        std::fs::create_dir_all(dir.join("session")).unwrap();
        Scratch(dir)
    }

    fn blobs(&self) -> Blobs {
        Blobs::new(self.0.join("blobs"))
    }

    fn output(&self, job: u64) -> PathBuf {
        miyu_store::jobs::output_path(&self.0.join("session"), &JobId::new(job).unwrap())
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 一张表、一个会话的那一份：编号从 `used` 往后数；交回 actor 的收件箱。
fn session(
    scratch: &Scratch,
    table: &Arc<Jobs>,
    used: u64,
) -> (SessionJobs, mpsc::UnboundedReceiver<Back>) {
    let (backs, back) = mpsc::unbounded_channel();
    let kit = Kit {
        table: Arc::clone(table),
        dir: scratch.0.join("session"),
        blobs: scratch.blobs(),
        ids: Arc::new(JobIds::starting_after(None, used)),
        roster: Roster::default(),
        agents: None,
    };
    let jobs = SessionJobs::new(kit, backs);
    (jobs, back)
}

fn call() -> CallId {
    CallId::parse("call_5_1").unwrap()
}

fn cause() -> Option<CommandId> {
    Some(CommandId::parse("cmd-1").unwrap())
}

fn at() -> Timestamp {
    Timestamp::from_unix_millis(0).unwrap()
}

/// 等收件箱里来一条结束，最多十秒。
async fn next_end(back: &mut mpsc::UnboundedReceiver<Back>) -> Ended {
    let waited = tokio::time::timeout(Duration::from_secs(10), back.recv()).await;
    match waited.expect("十秒内报了结束").expect("收件箱没关") {
        Back::Job(ended) => ended,
        other => panic!("只该有任务的结束：{other:?}"),
    }
}

/// 等到 `path` 的内容是 `want`，最多十秒。
async fn until_written(path: &Path, want: &str) {
    for _ in 0..2000 {
        if std::fs::read_to_string(path).is_ok_and(|text| text == want) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("十秒内 {} 没写成 {want:?}", path.display());
}

#[tokio::test]
async fn a_job_counts_on_writes_its_output_and_reports_when_it_exits() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (mut jobs, mut back) = session(&scratch, &table, 2);
    let held = Held::new(&["第一行\n", "second\n"]);
    let job = jobs.port(call(), cause()).start(held.background()).unwrap();
    assert_eq!(job.to_string(), "j3", "照日志里用过的往后数");
    until_written(&scratch.output(3), "第一行\nsecond\n").await;
    assert!(table.running(), "在跑");
    held.end(Exit::Code(3));
    let ended = next_end(&mut back).await;
    let Input::JobEnded {
        by,
        cause: got,
        reported,
        ..
    } = jobs.arrived(at(), ended)
    else {
        panic!("写成后台命令结束")
    };
    assert_eq!(by, By::Tool(Tool { call_id: call() }), "起它的那次调用");
    assert_eq!(got, cause(), "那一轮的 cause");
    assert_eq!(reported.job, job);
    assert_eq!(reported.reason, JobReason::Exited);
    assert_eq!(reported.exit_code, Some(3));
    assert_eq!(reported.signal, None);
    assert!(reported.duration_ms.is_some());
    let whole = "第一行\nsecond\n".as_bytes();
    assert_eq!(reported.output, Some(ContentHash::of(whole)));
    assert_eq!(reported.chars, Some(11), "按字数，不按字节");
    assert_eq!(scratch.blobs().get(&ContentHash::of(whole)).unwrap(), whole);
    assert!(table.running(), "落盘之前还算在跑");
    jobs.land();
    assert!(!table.running(), "落了盘就拿掉");
    assert_eq!(held.killed(), 0);
}

#[tokio::test]
async fn a_signal_is_reported_and_an_empty_output_has_no_characters() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, mut back) = session(&scratch, &table, 0);
    let held = Held::new(&[]);
    jobs.port(call(), None).start(held.background()).unwrap();
    held.end(Exit::Signal(15));
    let ended = next_end(&mut back).await;
    assert_eq!(ended.reported.job.to_string(), "j1");
    assert_eq!(ended.reported.signal, Some(15));
    assert_eq!(ended.reported.exit_code, None);
    assert_eq!(ended.reported.chars, Some(0));
    assert_eq!(ended.cause, None);
}

#[tokio::test]
async fn stopping_reports_the_running_ones_and_then_kills_only_them() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, mut back) = session(&scratch, &table, 0);
    let running = Held::new(&["half\n"]);
    let exited = Held::new(&[]);
    jobs.port(call(), cause())
        .start(running.background())
        .unwrap();
    jobs.port(call(), cause())
        .start(exited.background())
        .unwrap();
    until_written(&scratch.output(1), "half\n").await;
    exited.end(Exit::Code(0));
    // 等它报了（交进收件箱），再停下：它不再报成 restarted。
    let early = next_end(&mut back).await;
    assert_eq!(early.reported.job.to_string(), "j2");
    let restarted = jobs.restarted(at()).await;
    assert_eq!(restarted.len(), 1, "只报还在跑的：{restarted:?}");
    let Input::JobEnded {
        by,
        cause: got,
        reported,
        ..
    } = &restarted[0]
    else {
        panic!("写成后台命令结束")
    };
    assert_eq!(*by, By::Kernel);
    assert_eq!(*got, None);
    assert_eq!(reported.job.to_string(), "j1");
    assert_eq!(reported.reason, JobReason::Restarted);
    assert!(reported.duration_ms.is_some());
    assert_eq!(reported.output, Some(ContentHash::of(b"half\n")));
    assert_eq!(reported.exit_code, None, "报的时候还没杀");
    assert_eq!(running.killed(), 0, "落盘之前不杀");
    running.end(Exit::Signal(9));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        back.try_recv().is_err(),
        "报过 restarted 的，自己结束了也不再报"
    );
    let later = Held::new(&[]);
    jobs.port(call(), cause())
        .start(later.background())
        .unwrap();
    jobs.close().await;
    assert_eq!(later.killed(), 1, "还在跑的整组杀掉");
    assert_eq!(exited.killed(), 0, "已经结束的不杀");
    assert!(!table.running());
    let refused = Held::new(&[]);
    assert!(
        jobs.port(call(), cause())
            .start(refused.background())
            .is_err(),
        "停了以后不收新的"
    );
    assert_eq!(refused.killed(), 1, "收不下的整组杀掉");
}

#[tokio::test]
async fn an_actor_going_away_kills_its_jobs_and_leaves_the_others() {
    let scratch = Scratch::new();
    let other = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, _back) = session(&scratch, &table, 0);
    let (others, _other_back) = session(&other, &table, 0);
    let mine = Held::new(&[]);
    let theirs = Held::new(&[]);
    let port = jobs.port(call(), cause());
    port.start(mine.background()).unwrap();
    others
        .port(call(), cause())
        .start(theirs.background())
        .unwrap();
    drop(jobs);
    assert_eq!(mine.killed(), 1, "actor 停了，它的整组杀掉");
    assert_eq!(theirs.killed(), 0, "别的会话的不动");
    assert!(table.running(), "别的会话的还在跑");
    let late = Held::new(&[]);
    assert!(
        port.start(late.background()).is_err(),
        "actor 停了以后交来的不收"
    );
    assert_eq!(late.killed(), 1);
    drop(others);
    assert!(!table.running());
}

#[tokio::test]
async fn an_output_file_that_cannot_be_made_refuses_the_job() {
    let scratch = Scratch::new();
    // `jobs` 那里是个文件：建不了目录。
    std::fs::write(scratch.0.join("session").join("jobs"), b"").unwrap();
    let table = Arc::new(Jobs::new());
    let (jobs, _back) = session(&scratch, &table, 0);
    let held = Held::new(&[]);
    assert!(jobs.port(call(), cause()).start(held.background()).is_err());
    assert_eq!(held.killed(), 1, "收不下的整组杀掉");
    assert!(!table.running());
}

/// 她、人停的（施工 7-4）：还在表里、还没人报过的才停。
fn who() -> Who {
    Who {
        by: By::Tool(Tool { call_id: call() }),
        cause: cause(),
        why: Why::Stopped { by_model: true },
    }
}

#[tokio::test]
async fn stopping_after_it_ended_on_its_own_does_nothing() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, mut back) = session(&scratch, &table, 0);
    let held = Held::new(&[]);
    let job = jobs.port(call(), cause()).start(held.background()).unwrap();
    held.end(Exit::Code(0));
    // 自己退出的已经报了、还没落盘：表里还有它，停也不再杀、不再报第二条。
    let ended = next_end(&mut back).await;
    assert!(table.running(), "还没落盘，还在表里");
    assert!(jobs.stop_command(job, who()).await.is_none(), "只认先到的");
    assert_eq!(held.killed(), 0);
    assert_eq!(ended.reported.reason, JobReason::Exited);
}

#[tokio::test]
async fn a_stopped_job_is_not_reported_again_when_its_process_ends() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, mut back) = session(&scratch, &table, 0);
    let held = Held::new(&["x\n"]);
    let job = jobs.port(call(), cause()).start(held.background()).unwrap();
    until_written(&scratch.output(1), "x\n").await;
    let stopped = jobs.stop_command(job, who()).await.expect("在跑，停得了");
    assert_eq!(stopped.reported.reason, JobReason::Stopped);
    assert!(stopped.reported.by_model);
    assert_eq!(stopped.reported.chars, Some(2), "带到这时的输出");
    assert_eq!(held.killed(), 1);
    // 杀掉以后等着它的那一头照常去报：表里已经记成报了，收件箱里什么都不来。
    let later = tokio::time::timeout(Duration::from_millis(300), back.recv()).await;
    assert!(later.is_err(), "不交第二次：{later:?}");
}
