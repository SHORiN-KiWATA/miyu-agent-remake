//! mermaid 出图（`mermaid.md`「守着它的」，施工 W-4）：三种记号色都换得掉、底和框不填色；同一份源码第二次不
//! 重画，满了丢最早的那一张；空的、太长、画不出各说一句；第一次调之前不读字体、不碰磁盘；崩了当画不出。

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时资源目录：`software/mermaid/style.json` 写成 `text`。不写就是没有这份文件。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-mermaid-{}-{n}", std::process::id())))
    }

    fn write(&self, text: &str) {
        let dir = self.0.join("software").join("mermaid");
        std::fs::create_dir_all(&dir).expect("建得了目录");
        let mut file = std::fs::File::create(dir.join("style.json")).expect("建得了文件");
        file.write_all(text.as_bytes()).expect("写得进");
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

fn style_json(max_source: usize, keep: usize) -> String {
    format!(
        r##"{{"fonts":["sans-serif"],"marks":{{"text":"#010203","line":"#040506","label":"#070809"}},"max_source":{max_source},"keep":{keep}}}"##
    )
}

const FLOWCHART: &str =
    "flowchart TD\n  A[Start] --> B{Decision}\n  B -->|Yes| C[OK]\n  B -->|No| D[Cancel]";

/// SVG 里的第一个 `<rect>`：整张图的底。
fn first_rect(svg: &str) -> &str {
    let rect = &svg[svg.find("<rect").expect("有底")..];
    &rect[..rect.find('>').expect("标签闭合")]
}

#[test]
fn marker_colours_are_swappable_and_background_is_unfilled() {
    let scratch = Scratch::new();
    scratch.write(&style_json(65536, 64));
    let mermaid = Mermaid::new(&scratch.0);
    let rendered = mermaid.render(FLOWCHART).expect("画得出来");
    assert!(rendered.svg.starts_with("<svg"), "{}", rendered.svg);
    assert!(rendered.svg.contains(&rendered.marks.text));
    assert!(rendered.svg.contains(&rendered.marks.line));
    assert!(rendered.svg.contains(&rendered.marks.label));
    assert!(
        first_rect(&rendered.svg).contains(r#"fill="none""#),
        "底不填色：{}",
        first_rect(&rendered.svg)
    );
}

#[test]
fn same_source_is_served_from_cache() {
    let scratch = Scratch::new();
    scratch.write(&style_json(65536, 64));
    let mermaid = Mermaid::new(&scratch.0);
    let first = mermaid.render(FLOWCHART).expect("画得出来");
    assert_eq!(
        mermaid.cache.lock().expect("锁没中毒").len(),
        1,
        "画好了存一份"
    );
    let second = mermaid.render(FLOWCHART).expect("画得出来");
    assert_eq!(first, second);
    assert_eq!(
        mermaid.cache.lock().expect("锁没中毒").len(),
        1,
        "同一份源码第二次不重画，不会多存一份"
    );
}

#[test]
fn the_cache_drops_the_oldest_once_full() {
    let scratch = Scratch::new();
    scratch.write(&style_json(65536, 2));
    let mermaid = Mermaid::new(&scratch.0);
    let sources = [
        "flowchart TD\nA-->B",
        "flowchart TD\nA-->C",
        "flowchart TD\nA-->D",
    ];
    let hashes: Vec<Hash> = sources
        .iter()
        .map(|source| {
            mermaid.render(source).expect("画得出来");
            Sha256::digest(source.as_bytes()).into()
        })
        .collect();
    let cache = mermaid.cache.lock().expect("锁没中毒");
    assert_eq!(cache.len(), 2, "最多记 2 张");
    let kept: Vec<Hash> = cache.iter().map(|(hash, _)| *hash).collect();
    assert!(!kept.contains(&hashes[0]), "最早画的那一张丢了");
    assert!(
        kept.contains(&hashes[1]) && kept.contains(&hashes[2]),
        "后两张还在"
    );
}

#[test]
fn empty_source_is_rejected() {
    let scratch = Scratch::new();
    scratch.write(&style_json(65536, 64));
    let mermaid = Mermaid::new(&scratch.0);
    assert_eq!(mermaid.render("   \n\t").unwrap_err(), RenderError::Empty);
}

#[test]
fn source_over_the_limit_is_rejected() {
    let scratch = Scratch::new();
    scratch.write(&style_json(10, 64));
    let mermaid = Mermaid::new(&scratch.0);
    let error = mermaid.render(&"x".repeat(11)).unwrap_err();
    assert_eq!(error, RenderError::TooLong { max: 10 });
    // 正好 10 个字节的收，不算超过（去掉空白以后再量，前后空白不算进上限）。
    match mermaid
        .render(&format!("  {}  ", "x".repeat(10)))
        .unwrap_err()
    {
        RenderError::Failed(_) => {}
        other => panic!("10 个字节没超上限，该是画不出语法不对：{other:?}"),
    }
}

#[test]
fn unparseable_source_says_why() {
    let scratch = Scratch::new();
    scratch.write(&style_json(65536, 64));
    let mermaid = Mermaid::new(&scratch.0);
    match mermaid.render("not a diagram at all").unwrap_err() {
        RenderError::Failed(detail) => assert!(!detail.is_empty(), "库的原话不是空的"),
        other => panic!("该是画不出：{other:?}"),
    }
}

#[test]
fn fonts_and_style_json_are_not_read_before_the_first_call() {
    // 资源目录压根不存在：造 Mermaid 不碰磁盘，不会报错；读字体、读 style.json 都等第一次调 render。
    let missing = std::env::temp_dir().join("miyu-mermaid-definitely-missing-dir");
    let mermaid = Mermaid::new(&missing);
    assert!(mermaid.ready.get().is_none(), "造完了还没初始化");
    let error = mermaid.render(FLOWCHART).unwrap_err();
    assert_eq!(error, RenderError::NotReady);
    assert!(mermaid.ready.get().is_some(), "调过一次才初始化");
}

#[test]
fn broken_style_json_is_not_ready() {
    let scratch = Scratch::new();
    scratch.write("{ 不是 JSON");
    let mermaid = Mermaid::new(&scratch.0);
    assert_eq!(
        mermaid.render(FLOWCHART).unwrap_err(),
        RenderError::NotReady
    );
}

#[test]
fn panic_payloads_become_a_message() {
    let a = std::panic::catch_unwind(|| panic!("boom")).expect_err("panic 了");
    assert_eq!(panic_message(&a), "boom");
    let owned = "owned message".to_string();
    let b = std::panic::catch_unwind(move || std::panic::panic_any(owned)).expect_err("panic 了");
    assert_eq!(panic_message(&b), "owned message");
    let c = std::panic::catch_unwind(|| std::panic::panic_any(42_i32)).expect_err("panic 了");
    assert_eq!(panic_message(&c), "the drawing library panicked");
}

/// 真的 `resources/software/mermaid/style.json`：读得进来，数都站得住（施工 W-4）。
#[test]
fn the_real_style_json_reads_and_both_sample_diagrams_render() {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mermaid = Mermaid::new(&resources);
    let sequence = "sequenceDiagram\n  participant U as User\n  participant C as Core\n  U->>C: hello\n  C-->>U: ok";
    for source in [FLOWCHART, sequence] {
        let rendered = mermaid.render(source).expect("真的资源目录，画得出来");
        assert!(rendered.svg.starts_with("<svg"));
    }
}
