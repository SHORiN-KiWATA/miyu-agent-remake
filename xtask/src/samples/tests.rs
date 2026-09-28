use super::*;

/// 只有一份文件 `r/a.json` 的仓库。
fn read(file: &str) -> Result<Vec<u8>, String> {
    match file {
        "r/a.json" => Ok(b"{\"a\":1}\n\nsecond\n".to_vec()),
        other => Err(format!("没有 {other}")),
    }
}

const PAGE: &str = "\
## 一页

说明。

样本 `r/a.json`（原文）：

```json
{\"a\":1}

second
```

别的块不管：

```text
随便写
```
";

#[test]
fn a_block_that_matches_its_file_passes() {
    assert!(page("p.md", PAGE, read).is_empty());
}

#[test]
fn every_mismatch_is_named() {
    // 多一个字。
    let more = PAGE.replace("second\n```", "second!\n```");
    let problems = page("p.md", &more, read);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("p.md 第 5 行的样本和 r/a.json 对不上，从第 16 个字节起不一样"),
        "{problems:?}"
    );
    // 少一行空行。
    let fewer = PAGE.replace("{\"a\":1}\n\nsecond", "{\"a\":1}\nsecond");
    assert_eq!(page("p.md", &fewer, read).len(), 1);
    // 指到的文件没有。
    let missing = PAGE.replace("`r/a.json`", "`r/b.json`");
    let problems = page("p.md", &missing, read);
    assert!(
        problems[0].contains("指到 r/b.json，读不了"),
        "{problems:?}"
    );
    // 没写路径。
    let bare = PAGE.replace("样本 `r/a.json`（原文）：", "样本 （原文）：");
    let problems = page("p.md", &bare, read);
    assert!(problems[0].contains("没写指到哪份文件"), "{problems:?}");
    // 后面没有块。
    let blockless = PAGE.replace("```json\n{\"a\":1}\n\nsecond\n```\n", "只是一句话。\n");
    let problems = page("p.md", &blockless, read);
    assert!(problems[0].contains("后面没有块"), "{problems:?}");
    // 块没有收尾。
    let open = "样本 `r/a.json`：\n\n```json\n{\"a\":1}\n";
    let problems = page("p.md", open, read);
    assert!(problems[0].contains("没有收尾"), "{problems:?}");
}

#[test]
fn a_longer_fence_closes_only_on_its_own_length() {
    let text = "样本 `r/x.txt`：\n\n````text\n```\ninside\n```\n````\n";
    let read = |_: &str| Ok(b"```\ninside\n```\n".to_vec());
    assert!(page("p.md", text, read).is_empty());
}

#[test]
fn prose_that_only_starts_with_the_word_is_not_a_sample() {
    let text = "样本会话里有两轮。\n\n```text\n不比\n```\n";
    assert!(page("p.md", text, read).is_empty());
}

#[test]
fn every_page_under_the_blueprint_is_checked() {
    let dir = std::env::temp_dir().join(format!("xtask-samples-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("docs/blueprint/tools")).unwrap();
    std::fs::create_dir_all(dir.join("r")).unwrap();
    std::fs::write(dir.join("r/a.json"), "{\"a\":1}\n\nsecond\n").unwrap();
    std::fs::write(dir.join("docs/blueprint/tools/p.md"), PAGE).unwrap();
    std::fs::write(
        dir.join("docs/blueprint/notes.txt"),
        PAGE.replace("second", "x"),
    )
    .unwrap();
    assert!(check(&dir).is_empty(), "{:?}", check(&dir));
    // 子目录里的一页改了一边：报的是那一页。
    std::fs::write(
        dir.join("docs/blueprint/tools/p.md"),
        PAGE.replace("second\n```", "2nd\n```"),
    )
    .unwrap();
    let problems = check(&dir);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("docs/blueprint/tools/p.md 第 5 行"),
        "{problems:?}"
    );
    drop(std::fs::remove_dir_all(&dir));
}
