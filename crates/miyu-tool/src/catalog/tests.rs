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
