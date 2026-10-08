//! 软件包清单在哪、两层怎么认（施工 9-1 上）：出厂的、管理员家目录里的，照编号排；同一个编号认出厂的；子命令名撞了的
//! 后读到的那一份报错；读不了的、不是 `.toml` 的、编号不合写法的。

use std::fs;

use miyu_config::package::{Code, PackageKind};

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

/// 一个临时的资源目录和数据根：`res/`、`data/`。
struct Places {
    scratch: Scratch,
    packages: Packages,
}

impl Places {
    fn new() -> Places {
        let scratch = Scratch::new();
        let resources = ResourceRoot::at(scratch.path().join("res"));
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.path().join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).unwrap();
        let admin = AccountId::parse("admin").unwrap();
        let packages = Packages::new(&resources, &root, &admin);
        Places { scratch, packages }
    }

    /// 在一层里写一份文件。
    fn write(&self, layer: Layer, file: &str, text: &str) {
        let base = match layer {
            Layer::Shipped => "res/packages",
            Layer::Home => "data/home/admin/packages",
        };
        let path = self.scratch.path().join(base).join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

/// 一份最小的界面清单，子命令名是 `command`。
fn ui(command: &str) -> String {
    format!(
        "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = {{ en = \"{command}\" }}\n\n[command]\nname = \"{command}\"\nprogram = \"miyu-{command}\"\nabout = {{ en = \"Open {command}\" }}\n"
    )
}

/// 读到的：编号、层、读成了没有（问题的代码）。
fn brief(found: &[Found]) -> Vec<(String, Layer, Option<Code>)> {
    found
        .iter()
        .map(|found| {
            let code = match &found.read {
                Ok(_) => None,
                Err(Issue::Wrong(problem)) => Some(problem.code),
                Err(Issue::Unreadable(_)) => Some(Code::Syntax),
            };
            (found.id.clone(), found.layer, code)
        })
        .collect()
}

#[test]
fn both_layers_are_read_in_order_of_id() {
    let places = Places::new();
    places.write(Layer::Shipped, "web.toml", &ui("web"));
    places.write(Layer::Home, "tui.toml", &ui("tui"));
    places.write(Layer::Home, "notes.txt", "不是清单");
    places.write(Layer::Home, "Bad.toml", &ui("bad"));
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("tui".to_string(), Layer::Home, None),
            ("web".to_string(), Layer::Shipped, None),
        ],
        "不是 .toml 的、编号不合写法的不算"
    );
    let web = found.iter().find(|found| found.id == "web").unwrap();
    assert_eq!(web.read.as_ref().unwrap().kind, PackageKind::Ui);
    assert!(web.path.ends_with("res/packages/web.toml"));
}

#[test]
fn nothing_installed_is_an_empty_list() {
    assert!(Places::new().packages.read().is_empty());
}

#[test]
fn the_shipped_one_wins_a_duplicate_id() {
    let places = Places::new();
    places.write(Layer::Shipped, "web.toml", &ui("web"));
    places.write(Layer::Home, "web.toml", &ui("web2"));
    assert_eq!(
        brief(&places.packages.read()),
        [
            ("web".to_string(), Layer::Shipped, None),
            ("web".to_string(), Layer::Home, Some(Code::Duplicate)),
        ]
    );
}

#[test]
fn a_command_name_is_taken_by_whoever_is_read_first() {
    let places = Places::new();
    places.write(Layer::Shipped, "web.toml", &ui("open"));
    places.write(Layer::Shipped, "zeta.toml", &ui("open"));
    places.write(Layer::Home, "alpha.toml", &ui("open"));
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("alpha".to_string(), Layer::Home, Some(Code::CommandTaken)),
            ("web".to_string(), Layer::Shipped, None),
            ("zeta".to_string(), Layer::Shipped, Some(Code::CommandTaken)),
        ],
        "出厂的先于家目录，同一层照编号"
    );
    let Err(Issue::Wrong(problem)) = &found[0].read else {
        panic!("撞了");
    };
    assert_eq!(problem.detail, "open");
    assert_eq!(problem.line, Some(7), "报在子命令名那一行");
}

/// 一份最小的扩展清单，声明了系统账号。
fn served(command: &str) -> String {
    format!(
        "{}\n[process]\nsystem_account = true\n",
        ui(command).replace("kind = \"ui\"", "kind = \"process\"")
    )
}

/// 系统账号（施工 O-4 下）：读成了的、声明了的包各一个，账号名是编号；编号是管理员的那一份报 `account_taken`、整份不收。
#[test]
fn a_package_declaring_a_system_account_gets_one_named_after_it() {
    let places = Places::new();
    places.write(Layer::Shipped, "onebot.toml", &served("onebot"));
    places.write(Layer::Home, "admin.toml", &served("boss"));
    places.write(Layer::Home, "tui.toml", &ui("tui"));
    places.write(Layer::Home, "zz.toml", "[package]\nkind = \"daemon\"\n");
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("admin".to_string(), Layer::Home, Some(Code::AccountTaken)),
            ("onebot".to_string(), Layer::Shipped, None),
            ("tui".to_string(), Layer::Home, None),
            ("zz".to_string(), Layer::Home, Some(Code::BadKind)),
        ]
    );
    assert_eq!(
        system_accounts(&found),
        [AccountId::parse("onebot").unwrap()],
        "没声明的、写错的不算"
    );
    let Err(Issue::Wrong(problem)) = &found[0].read else {
        panic!("撞了管理员");
    };
    assert_eq!(problem.detail, "admin");
    assert_eq!(problem.line, None);
}

#[test]
fn a_broken_manifest_is_listed_with_its_problem() {
    let places = Places::new();
    places.write(Layer::Shipped, "web.toml", "[package]\nkind = \"daemon\"\n");
    assert_eq!(
        brief(&places.packages.read()),
        [("web".to_string(), Layer::Shipped, Some(Code::BadKind))]
    );
}

#[test]
fn the_state_dir_is_under_the_state_root() {
    let places = Places::new();
    assert!(
        places
            .packages
            .state_dir("tui")
            .ends_with("data/state/packages/tui")
    );
}

/// 每一种问题（加上协议版本对不上的那一句）三种语言都有给人看的一句，`{detail}` 换得进去（施工 9-1 上、下）。
#[test]
fn every_problem_code_is_said_in_three_languages() {
    let resources =
        ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let mut codes: Vec<&str> = Code::ALL.iter().map(|code| code.as_str()).collect();
    let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(unique.len(), codes.len(), "写法不重复");
    codes.push("protocol_mismatch");
    // 跑包自己的检查时出的（施工 9-2，`miyu-endpoint` 的 `check/run.rs`）。
    codes.extend(["check_failed", "check_unavailable", "check_output"]);
    for language in ["zh", "en", "ja"] {
        let human = crate::human::Human::load(&resources, language).unwrap();
        for code in &codes {
            let said = miyu_config::Words::sentence(
                &human,
                &format!("package-problems/{code}"),
                &[("detail", "X")],
            );
            assert!(said.is_some(), "{language} 少了 {code}");
        }
    }
}
