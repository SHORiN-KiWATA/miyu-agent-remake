use std::path::{Path, PathBuf};

use miyu_store::env::{Env, Platform};

use super::*;

/// 资源目录指到 `resources`、家目录是 `home` 的环境。
fn env(resources: PathBuf, home: Option<PathBuf>) -> Env {
    Env {
        platform: Platform::current(),
        miyu_home: None,
        home,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: Some(resources.into_os_string()),
        exe: None,
    }
}

/// `miyu ask` 后面跟着 `words`，别的都不写。
fn ask(words: &[&str]) -> Ask {
    Ask {
        words: words.iter().map(|word| (*word).to_string()).collect(),
        session: None,
        resume: false,
        format: Format::Text,
        add_dir: Vec::new(),
    }
}

/// 读的那件工具给人看的显示名。
fn read_name(plan: &Plan) -> Option<&str> {
    plan.human.tool("read").map(|face| face.name.as_str())
}

#[test]
fn the_plan_reads_the_human_texts_in_the_interface_language() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let home = PathBuf::from("/home/someone");
    let chinese = plan(
        ask(&["读", "一下"]),
        &env(resources.clone(), Some(home.clone())),
        Language::Chinese,
    );
    assert_eq!(chinese.text, "读 一下");
    assert_eq!(chinese.target, Target::New);
    assert_eq!(chinese.home, Some(home), "路径照它写成 ~");
    assert_eq!(read_name(&chinese), Some("读取"));
    let english = plan(ask(&["hi"]), &env(resources, None), Language::English);
    assert_eq!(read_name(&english), Some("Read"));
    // 读不出来的当没有：每一步照状态写最泛的一句。
    let nowhere = std::env::temp_dir().join("miyu-cli-no-resources-here");
    let bare = plan(ask(&["hi"]), &env(nowhere, None), Language::Chinese);
    assert_eq!(read_name(&bare), None);
}

/// 加进来的目录（施工 5-10 上）：照写的先后，去掉重复的；没有的是空的。
#[test]
fn added_dirs_keep_their_order_without_repeats() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let (a, b) = (PathBuf::from("/work/a"), PathBuf::from("/work/b"));
    let args = Ask {
        add_dir: vec![a.clone(), b.clone(), a.clone()],
        ..ask(&["hi"])
    };
    let planned = plan(args, &env(resources.clone(), None), Language::English);
    assert_eq!(
        planned.dirs,
        [
            a.to_string_lossy().into_owned(),
            b.to_string_lossy().into_owned()
        ]
    );
    let none = plan(ask(&["hi"]), &env(resources, None), Language::English);
    assert!(none.dirs.is_empty());
}

/// `--add-dir` 读参数时就换成绝对的：相对的接在敲命令时的目录上；不是目录的读不成。
#[test]
fn an_added_dir_is_made_absolute_and_must_be_a_directory() {
    let here = std::env::current_dir().expect("有工作目录");
    assert_eq!(directory("."), Ok(here.join(".")));
    assert!(directory("no-such-dir-for-miyu-cli-tests").is_err());
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    assert!(directory(&file.to_string_lossy()).is_err(), "文件不算");
}
