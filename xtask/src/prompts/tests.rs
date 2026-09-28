use std::collections::BTreeMap;

use super::*;

const LEDGER: &str = "\
### 十、登记簿

| 文件 | 进到哪 | 什么时候加进来 | token | 为什么加 | 指纹 |
|---|---|---|---|---|---|
| `core/a.txt` | 事实 | 回合开始 | 3 | 施工 3-5 | `2cf24dba` |
| `tools/b.json` | tools 数组 | 每次请求 | 约 9（估的） | 施工 4-4 | `00000000` |
| `core/c.txt` | 事实 | 断了的那一次 | 1 | 施工 3-5 下 | `11111111` |

### 附录
";

fn texts(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(file, text)| ((*file).to_string(), (*text).to_string()))
        .collect()
}

#[test]
fn the_page_groups_by_place_in_ledger_order() {
    let rows = rows(LEDGER).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].tokens, "约 9（估的）");
    let page = page(
        &rows,
        &texts(&[
            ("core/a.txt", "<a>hello</a>\n"),
            ("tools/b.json", "{\"description\":\"x\"}\n"),
            ("core/c.txt", "no newline at the end"),
        ]),
    );
    // 组照第一次出现的先后：事实在前，组里照登记簿的先后（c 排在 a 后面，不跟着 b）。
    let facts = page.find("\n### 事实\n").unwrap();
    let tools = page.find("\n### tools 数组\n").unwrap();
    let a = page.find("#### `core/a.txt`").unwrap();
    let c = page.find("#### `core/c.txt`").unwrap();
    assert!(facts < a && a < c && c < tools, "{page}");
    // 每一组、每一份只写一次。
    assert_eq!(page.matches("\n### 事实\n").count(), 1, "{page}");
    assert_eq!(page.matches("#### `core/a.txt`").count(), 1, "{page}");
    assert!(page.contains(
        "#### `core/a.txt`\n\n- 什么时候加进来：回合开始\n- token：3\n- 为什么加：施工 3-5\n- 指纹：`2cf24dba`\n\n```text\n<a>hello</a>\n```\n"
    ), "{page}");
    assert!(
        page.contains("```json\n{\"description\":\"x\"}\n```\n"),
        "{page}"
    );
    // 末尾没有换行的照原样放。
    assert!(
        page.contains("```text\nno newline at the end\n```\n"),
        "{page}"
    );
    assert!(
        page.starts_with("## 给模型看的字\n\n这一页是生成的"),
        "{page}"
    );
}

#[test]
fn a_text_with_backticks_gets_a_longer_fence() {
    assert_eq!(fenced("a.txt", "plain\n"), "```text\nplain\n```\n");
    assert_eq!(
        fenced("a.txt", "see ```code``` and ``x``\n"),
        "````text\nsee ```code``` and ``x``\n````\n"
    );
}

#[test]
fn a_row_needs_six_cells_and_backticked_names() {
    let short = LEDGER.replace("| 3 | 施工 3-5 | `2cf24dba` |", "| 3 | `2cf24dba` |");
    assert!(rows(&short).unwrap_err().contains("6 格"));
    let bare = LEDGER.replace("| `core/a.txt` |", "| core/a.txt |");
    assert!(rows(&bare).unwrap_err().contains("反引号"));
}

/// 一个用完就删的临时仓库：登记簿、几份资源。
struct Repo(std::path::PathBuf);

impl Repo {
    fn new(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs/designs")).unwrap();
        std::fs::create_dir_all(dir.join("docs/blueprint")).unwrap();
        std::fs::create_dir_all(dir.join("resources/core")).unwrap();
        std::fs::create_dir_all(dir.join("resources/tools")).unwrap();
        std::fs::write(dir.join(crate::ledger::PATH), LEDGER).unwrap();
        std::fs::write(dir.join("resources/core/a.txt"), "a\n").unwrap();
        std::fs::write(dir.join("resources/tools/b.json"), "{}\n").unwrap();
        std::fs::write(dir.join("resources/core/c.txt"), "c\n").unwrap();
        Repo(dir)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

#[test]
fn the_check_asks_to_regenerate_when_anything_moved() {
    let repo = Repo::new("prompts");
    // 还没生成：说要生成。
    assert!(check(&repo.0)[0].contains("跑 cargo xtask prompts 生成"));
    write(&repo.0).unwrap();
    assert!(check(&repo.0).is_empty());
    // 资源改了、登记簿改了、页面手改了，都说要重新生成。
    std::fs::write(repo.0.join("resources/core/a.txt"), "changed\n").unwrap();
    assert!(check(&repo.0)[0].contains("重新生成"));
    write(&repo.0).unwrap();
    let ledger = LEDGER.replace("| 3 |", "| 4 |");
    std::fs::write(repo.0.join(crate::ledger::PATH), ledger).unwrap();
    assert!(check(&repo.0)[0].contains("重新生成"));
    write(&repo.0).unwrap();
    let page = std::fs::read_to_string(repo.0.join(PATH)).unwrap();
    std::fs::write(
        repo.0.join(PATH),
        page.replace("## 给模型看的字", "## 手改过"),
    )
    .unwrap();
    assert!(check(&repo.0)[0].contains("重新生成"));
}
