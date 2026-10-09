use std::sync::Arc;

use miyu_kernel::tool::Access;

use super::*;
use crate::testkit::{Act, Fake};

fn tool(name: &str, parameters: &str) -> Arc<dyn Tool> {
    Fake::with_parameters(name, Access::Read, parameters, Act::Echo)
}

fn object(name: &str) -> Arc<dyn Tool> {
    tool(name, r#"{"type":"object","properties":{}}"#)
}

fn names(catalog: &Catalog) -> Vec<&str> {
    catalog.specs().map(|spec| spec.name.as_str()).collect()
}

#[test]
fn the_tools_come_out_by_name_whatever_order_they_came_in() {
    let catalog = Catalog::new([object("read"), object("grep"), object("edit")]).unwrap();
    assert_eq!(names(&catalog), ["edit", "grep", "read"]);
    let again = Catalog::new([object("edit"), object("read"), object("grep")]).unwrap();
    assert_eq!(names(&again), names(&catalog));
    assert_eq!(format!("{catalog:?}"), r#"["edit", "grep", "read"]"#);
}

#[test]
fn an_empty_catalog_has_no_tools() {
    assert_eq!(Catalog::new([]).unwrap().specs().count(), 0);
    assert_eq!(Catalog::default().specs().count(), 0);
}

#[test]
fn a_second_tool_with_the_same_name_is_refused() {
    let error = Catalog::new([object("read"), object("grep"), object("read")]).unwrap_err();
    assert_eq!(
        error,
        CatalogError {
            tool: "read".to_string(),
            problem: Problem::Duplicate,
        }
    );
    assert_eq!(
        error.to_string(),
        r#"tool "read": another tool has the same name"#
    );
}

#[test]
fn a_name_providers_would_refuse_is_refused() {
    let long = "a".repeat(65);
    for name in [
        "",
        "read file",
        "读取",
        "read.file",
        "read/file",
        long.as_str(),
    ] {
        let error = Catalog::new([object(name)]).unwrap_err();
        assert_eq!(error.problem, Problem::Name, "{name:?}");
        assert_eq!(error.tool, name);
    }
    let longest = "a".repeat(64);
    for name in [
        "read",
        "message_agent",
        "web-fetch",
        "Tool2",
        longest.as_str(),
    ] {
        assert!(Catalog::new([object(name)]).is_ok(), "{name:?}");
    }
}

#[test]
fn parameters_that_are_not_an_object_schema_are_refused() {
    for parameters in [
        r#"{"type":"string"}"#,
        r#"{"properties":{}}"#,
        r#"{"type":["object","null"]}"#,
        r#"["object"]"#,
        r#""object""#,
    ] {
        let error = Catalog::new([tool("read", parameters)]).unwrap_err();
        assert_eq!(error.problem, Problem::Parameters, "{parameters}");
    }
    assert!(Catalog::new([tool("read", r#"{"type":"object"}"#)]).is_ok());
}

#[test]
fn the_first_tool_that_fails_is_the_one_reported() {
    let error = Catalog::new([object("bad name"), object("read"), object("read")]).unwrap_err();
    assert_eq!(error.tool, "bad name");
    assert_eq!(error.problem, Problem::Name);
}

#[test]
fn the_parameters_are_kept_byte_for_byte() {
    let text = r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#;
    let catalog = Catalog::new([tool("read", text)]).unwrap();
    let spec = catalog.specs().next().unwrap();
    assert_eq!(spec.parameters.get(), text);
}

/// 改过名的一件（施工 7-5 再补）：规格、执行照 `tool`，以前叫 `formerly`。
struct Former(Arc<dyn Tool>, &'static [&'static str]);

impl Tool for Former {
    fn spec(&self) -> &Spec {
        self.0.spec()
    }

    fn run(&self, call: crate::Call, progress: crate::Progress) -> crate::Running<'_> {
        self.0.run(call, progress)
    }

    fn formerly(&self) -> &'static [&'static str] {
        self.1
    }
}

fn former(name: &str, formerly: &'static [&'static str]) -> Arc<dyn Tool> {
    Arc::new(Former(object(name), formerly))
}

#[test]
fn a_renamed_tool_is_found_by_its_old_name_but_offered_only_by_its_new_one() {
    let catalog = Catalog::new([object("read"), former("subagent", &["agent"])]).unwrap();
    assert_eq!(
        names(&catalog),
        ["read", "subagent"],
        "以前的名字不进工具面"
    );
    let found = catalog.get("agent").expect("照以前的名字找得到");
    assert_eq!(found.spec().name, "subagent");
    assert_eq!(catalog.get("subagent").unwrap().spec().name, "subagent");
    assert!(catalog.get("read").is_some());
    assert!(catalog.get("agents").is_none());
}

#[test]
fn an_old_name_that_clashes_is_refused() {
    let duplicate = |tool: &str| CatalogError {
        tool: tool.to_string(),
        problem: Problem::Duplicate,
    };
    // 以前的名字撞上前面登记的：现在的名字、以前的名字。
    let taken = Catalog::new([object("agent"), former("subagent", &["agent"])]);
    assert_eq!(taken.unwrap_err(), duplicate("agent"));
    let twice = Catalog::new([former("a", &["old"]), former("b", &["old"])]);
    assert_eq!(twice.unwrap_err(), duplicate("old"));
    // 后登记的现在的名字撞上前面的以前的名字。
    let later = Catalog::new([former("subagent", &["agent"]), object("agent")]);
    assert_eq!(later.unwrap_err(), duplicate("agent"));
}

#[test]
fn tools_remember_which_package_they_came_in() {
    let catalog = Catalog::in_packages([
        (
            "basesystem",
            vec![object("read"), former("subagent", &["agent"])],
        ),
        ("memory", vec![object("remember")]),
    ])
    .unwrap();
    assert_eq!(catalog.package_of("read"), Some("basesystem"));
    assert_eq!(catalog.package_of("remember"), Some("memory"));
    assert_eq!(
        catalog.package_of("agent"),
        Some("basesystem"),
        "以前的名字照现在的那一件"
    );
    assert_eq!(catalog.package_of("nope"), None);
    assert_eq!(
        catalog.packages().collect::<Vec<_>>(),
        ["basesystem", "memory"]
    );
    let plain = Catalog::new([object("read")]).unwrap();
    assert_eq!(
        plain.package_of("read"),
        Some(BASESYSTEM),
        "只交一串工具的全算基础系统"
    );
    let clash = Catalog::in_packages([
        ("basesystem", vec![object("read")]),
        ("other", vec![object("read")]),
    ])
    .unwrap_err();
    assert_eq!(clash.problem, Problem::Duplicate, "两个包里同名的照样拒");
}

/// 换掉一个包的工具（施工 O-2 上：提供者再登记一次）：交回新的一份，原来那份不动；别的包的照留；撞上别的包的、写法不对的
/// 整个不收。
#[test]
fn replacing_a_package_keeps_the_others_and_checks_the_new_ones() {
    let catalog = Catalog::in_packages([
        ("basesystem", vec![object("read")]),
        ("onebot", vec![object("send")]),
    ])
    .unwrap();
    let swapped = catalog
        .replacing("onebot", vec![object("send_group"), object("recall")])
        .unwrap();
    assert_eq!(names(&swapped), ["read", "recall", "send_group"]);
    assert_eq!(swapped.package_of("recall"), Some("onebot"));
    assert_eq!(names(&catalog), ["read", "send"], "原来那份不动");
    let fresh = catalog.replacing("voice", vec![object("speak")]).unwrap();
    assert_eq!(names(&fresh), ["read", "send", "speak"], "新的包加进来");
    let taken = catalog
        .replacing("onebot", vec![object("read")])
        .unwrap_err();
    assert_eq!(
        (taken.tool.as_str(), taken.problem),
        ("read", Problem::Duplicate)
    );
    let bad = catalog
        .replacing("onebot", vec![object("bad name")])
        .unwrap_err();
    assert_eq!(bad.problem, Problem::Name);
}

/// 经提供者登记的包目录自己记着（施工 O-2 中）：回合开头换快照时，只有它们的工具照现在的登记换。登记成空的，那几件就没了。
#[test]
fn the_catalog_knows_which_tools_came_from_a_provider() {
    let catalog = Catalog::in_packages([("basesystem", vec![object("read")])]).unwrap();
    assert!(!catalog.provided("read"), "起来时登记的不是");
    let provided = catalog.replacing("onebot", vec![object("send")]).unwrap();
    assert!(provided.provided("send"));
    assert!(!provided.provided("read"));
    assert!(!provided.provided("nothing"), "没有的不是");
    let emptied = provided.replacing("onebot", Vec::new()).unwrap();
    assert_eq!(names(&emptied), ["read"]);
    assert_eq!(emptied.packages().collect::<Vec<_>>(), ["basesystem"]);
}

/// 内置包装上、卸掉（施工 F-5 中）：卸掉的拿掉、记下随包卸掉了；装回来换上、不再算卸掉；都不算经提供者登记的。
#[test]
fn a_builtin_package_is_placed_and_removed_without_becoming_provided() {
    let catalog = Catalog::in_packages([
        ("basesystem", vec![object("read")]),
        ("memory", vec![object("remember"), object("forget")]),
    ])
    .unwrap();
    let removed = catalog.removing("memory");
    assert_eq!(names(&removed), ["read"]);
    assert!(removed.gone("remember") && removed.gone("forget"));
    assert!(!removed.gone("read"), "别的包的不算");
    assert!(!removed.provided("read"));
    let back = removed.placing("memory", vec![object("remember")]).unwrap();
    assert_eq!(names(&back), ["read", "remember"]);
    assert!(
        !back.gone("remember") && !back.gone("forget"),
        "装回来不再算卸掉"
    );
    assert!(!back.provided("remember"), "内置的不算提供者登记的");
    assert_eq!(back.package_of("remember"), Some("memory"));
    let clash = back.placing("other", vec![object("read")]);
    assert!(clash.is_err(), "撞了别的包照样拒");
}

/// 卸掉的扩展装回来、又登记了（施工 F-5 补）：它的几件不再算随包卸掉的，之后关掉照「用不了」说。
#[test]
fn a_provider_registering_again_is_no_longer_gone() {
    let catalog = Catalog::in_packages([("basesystem", vec![object("read")])]).unwrap();
    let provided = catalog
        .replacing("xbridge", vec![object("echo_back")])
        .unwrap();
    let removed = provided.removing("xbridge");
    assert!(removed.gone("echo_back"));
    let back = removed
        .replacing("xbridge", vec![object("echo_back")])
        .unwrap();
    assert!(!back.gone("echo_back"), "装回来登记了不再算卸掉");
    let off = back.replacing("xbridge", Vec::new()).unwrap();
    assert!(!off.gone("echo_back"), "关掉的不算卸掉");
}
