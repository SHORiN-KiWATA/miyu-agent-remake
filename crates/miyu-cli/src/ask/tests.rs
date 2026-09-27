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
