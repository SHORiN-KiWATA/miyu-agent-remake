use std::io::Write;

use super::*;

/// 一个用完就删的临时资源目录，`software/mermaid/style.json` 写成 `text`（不写就是没有这份文件）。
pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new() -> Scratch {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-mermaid-style-{}-{n}", std::process::id())))
    }

    pub(crate) fn write(&self, text: &str) {
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

const GOOD: &str = r##"{"fonts":["sans-serif"],"marks":{"text":"#010203","line":"#040506","label":"#070809"},"max_source":100,"keep":2}"##;

#[test]
fn a_good_file_reads() {
    let scratch = Scratch::new();
    scratch.write(GOOD);
    let style = load(&scratch.0).expect("读得出来");
    assert_eq!(style.fonts, vec!["sans-serif".to_string()]);
    assert_eq!(style.marks.text, "#010203");
    assert_eq!(style.max_source, 100);
    assert_eq!(style.keep, 2);
}

#[test]
fn a_missing_file_says_so() {
    let scratch = Scratch::new();
    let error = load(&scratch.0).unwrap_err();
    assert!(error.to_string().contains("style.json"), "{error}");
}

#[test]
fn broken_json_says_so() {
    let scratch = Scratch::new();
    scratch.write("{");
    load(&scratch.0).unwrap_err();
}

#[test]
fn an_unknown_field_is_rejected() {
    let scratch = Scratch::new();
    scratch.write(r#"{"fonts":["x"],"marks":{"text":"a","line":"b","label":"c"},"max_source":1,"keep":1,"extra":true}"#);
    load(&scratch.0).unwrap_err();
}

#[test]
fn empty_fonts_zero_max_source_or_zero_keep_are_rejected() {
    let scratch = Scratch::new();
    scratch.write(
        r#"{"fonts":[],"marks":{"text":"a","line":"b","label":"c"},"max_source":1,"keep":1}"#,
    );
    load(&scratch.0).unwrap_err();
    scratch.write(
        r#"{"fonts":["x"],"marks":{"text":"a","line":"b","label":"c"},"max_source":0,"keep":1}"#,
    );
    load(&scratch.0).unwrap_err();
    scratch.write(
        r#"{"fonts":["x"],"marks":{"text":"a","line":"b","label":"c"},"max_source":1,"keep":0}"#,
    );
    load(&scratch.0).unwrap_err();
}
