use std::sync::Arc;

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;

use super::*;

/// 一件只报规格的假工具。
struct Fake(Spec);

impl Tool for Fake {
    fn spec(&self) -> &Spec {
        &self.0
    }
}

fn tool(name: &str, parameters: &str) -> Arc<dyn Tool> {
    Arc::new(Fake(Spec {
        name: name.to_string(),
        description: format!("The {name} tool."),
        parameters: serde_json::from_str::<RawJson>(parameters).unwrap(),
        access: Access::Read,
    }))
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
