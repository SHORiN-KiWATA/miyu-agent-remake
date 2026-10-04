//! `cargo xtask package-web`（施工 W-11）：摆法、发布前检查（设计 12 R14）。

use std::path::{Path, PathBuf};

use super::{PROGRAM, check, lay_out};

/// 用完就删的临时目录。
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!("miyu-xtask-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("删得掉上一次的");
        }
        std::fs::create_dir_all(&dir).expect("建得了");
        Temp(dir)
    }
}

impl Drop for Temp {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    std::fs::write(path, text).expect("写得进");
}

/// 仓库里的 `resources/web/web.json`。
fn web_json() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("有上一级");
    std::fs::read_to_string(root.join("resources/web/web.json")).expect("读得到")
}

/// 摆一套齐的：程序（假的，几个字节）、`web.json`、页面。
fn complete(temp: &Temp) -> PathBuf {
    let program = temp.0.join("built").join(PROGRAM);
    write(&program, "fake");
    let resources = temp.0.join("resources");
    write(&resources.join("web/web.json"), &web_json());
    let pages = temp.0.join("pages");
    write(&pages.join("index.html"), "<h1>miyu</h1>");
    write(&pages.join("src/app.js"), "let x = 1;");
    lay_out(&program, &resources, &pages, &temp.0.join("out"), "1.2.3").expect("摆得成")
}

fn says(version: &str) -> impl Fn(&Path) -> Result<String, String> {
    let line = format!("miyu-web {version}\n");
    move |_| Ok(line.clone())
}

#[test]
fn it_lays_out_like_an_installed_package() {
    let temp = Temp::new("layout");
    let dir = complete(&temp);
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .expect("有名字");
    assert!(name.starts_with("miyu-web-1.2.3-"), "{name}");
    assert!(dir.join("bin").join(PROGRAM).is_file());
    assert_eq!(
        std::fs::read_to_string(dir.join("share/miyu/web/web.json")).expect("在"),
        web_json()
    );
    assert!(dir.join("share/miyu/web/pages/index.html").is_file());
    assert!(
        dir.join("share/miyu/web/pages/src/app.js").is_file(),
        "下一层的也搬"
    );
    assert_eq!(check(&dir, "1.2.3", says("1.2.3")), Vec::<String>::new());
    // 再摆一次：旧的整个换掉，不留上一次的东西。
    write(&dir.join("share/miyu/web/pages/stale.js"), "old");
    let again = complete(&temp);
    assert_eq!(again, dir);
    assert!(!dir.join("share/miyu/web/pages/stale.js").exists());
}

#[test]
fn no_index_page_is_not_laid_out() {
    let temp = Temp::new("noindex");
    let program = temp.0.join(PROGRAM);
    write(&program, "fake");
    let resources = temp.0.join("resources");
    write(&resources.join("web/web.json"), &web_json());
    let pages = temp.0.join("pages");
    write(&pages.join("other.html"), "x");
    let error =
        lay_out(&program, &resources, &pages, &temp.0.join("out"), "1.2.3").expect_err("摆不成");
    assert!(error.contains("index.html"), "{error}");
}

#[test]
fn the_check_names_what_is_wrong() {
    let temp = Temp::new("check");
    let dir = complete(&temp);
    assert_eq!(
        check(&dir, "1.2.4", says("1.2.3")),
        [format!(
            "bin/{PROGRAM} 报的版本是「miyu-web 1.2.3」，应该是「miyu-web 1.2.4」"
        )]
    );
    let broken = check(&dir, "1.2.3", |_| Err("Exec format error".to_string()));
    assert_eq!(
        broken,
        [format!("bin/{PROGRAM} 跑不起来：Exec format error")]
    );

    std::fs::remove_file(dir.join("share/miyu/web/pages/index.html")).expect("删得掉");
    std::fs::write(dir.join("share/miyu/web/web.json"), "{\"port\": 1}").expect("写得进");
    let problems = check(&dir, "1.2.3", says("1.2.3"));
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems[0].starts_with("share/miyu/web/web.json 读不懂："),
        "{problems:?}"
    );
    assert_eq!(problems[1], "少了 share/miyu/web/pages/index.html");

    std::fs::remove_file(dir.join("share/miyu/web/web.json")).expect("删得掉");
    std::fs::remove_file(dir.join("bin").join(PROGRAM)).expect("删得掉");
    let problems = check(&dir, "1.2.3", |_| panic!("没有程序就不跑"));
    assert_eq!(problems[0], format!("少了 bin/{PROGRAM}"));
    assert!(
        problems[1].starts_with("share/miyu/web/web.json 读不懂："),
        "{problems:?}"
    );
}

#[test]
fn web_json_must_know_html() {
    let temp = Temp::new("html");
    let dir = complete(&temp);
    let text = web_json().replace("\"html\":", "\"htm\":");
    std::fs::write(dir.join("share/miyu/web/web.json"), text).expect("写得进");
    assert_eq!(
        check(&dir, "1.2.3", says("1.2.3")),
        ["share/miyu/web/web.json 的 types 里没有 html：页面给不出去"]
    );
}
