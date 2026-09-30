//! `@` 文件列表的状态（蓝图 `tui.md`「`@` 文件列表」第 1、2 条）：跟着光标前面的词开关，`Esc` 关掉以后词没变就一直关着；
//! 只打了 `@` 列这一层，打名字模糊找（清单在后台建）。

use std::time::{Duration, Instant};

use super::{Mention, Status};
use crate::config::Config;

#[test]
fn the_list_follows_the_word_and_esc_keeps_it_shut() {
    let root = std::env::temp_dir().join(format!("miyu-mention-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), b"x").unwrap();
    std::fs::write(root.join("README.md"), b"x").unwrap();
    let mut m = Mention::new(Config::builtin().unwrap().mention);
    let at = |m: &mut Mention, text: &str| m.find(text, text.len(), &root, None);
    assert!(at(&mut m, "看看").is_none());
    let layer = at(&mut m, "看看 @").unwrap();
    assert_eq!(layer.start, "看看 ".len());
    assert_eq!(layer.status, Status::Layer, "只打了 @：列这一层");
    let shown: Vec<_> = layer.items.iter().map(|c| c.shown.as_str()).collect();
    assert_eq!(shown, ["src/", "README.md"]);
    // 打名字：模糊找，清单建好以前写「找文件中」。
    let until = Instant::now() + Duration::from_secs(5);
    let found = loop {
        let found = at(&mut m, "@main").unwrap();
        if found.status != Status::Indexing || Instant::now() > until {
            break found;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(found.status, Status::Full);
    assert_eq!(found.items[0].shown, "src/main.rs");
    assert_eq!(found.items[0].path, root.join("src/main.rs"));
    m.dismiss(&found);
    assert!(at(&mut m, "@main").is_none(), "Esc 关掉，词没变就一直关着");
    assert!(at(&mut m, "@mai").is_some(), "词变了照常开");
    std::fs::remove_dir_all(&root).unwrap_or_default();
}
