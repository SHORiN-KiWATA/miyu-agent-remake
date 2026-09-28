use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

#[test]
fn the_command_goes_after_the_spec_and_the_dashes() {
    let sandboxed = Sandboxed {
        helper: PathBuf::from("/opt/miyu/miyu-sandbox"),
        spec: Spec {
            write: vec![PathBuf::from("/work")],
            hidden: Vec::new(),
        },
        env: Vec::new(),
    };
    let args: Vec<OsString> = vec!["-c".into(), "echo hi -- there".into()];
    let (helper, wrapped) = argv(&sandboxed, Path::new("/bin/bash"), &args).expect("包得成");
    assert_eq!(helper, PathBuf::from("/opt/miyu/miyu-sandbox"));
    let expected: Vec<OsString> = vec![
        "run".into(),
        "--spec".into(),
        sandboxed.spec.to_json().expect("写得成").into(),
        "--".into(),
        "/bin/bash".into(),
        "-c".into(),
        "echo hi -- there".into(),
    ];
    assert_eq!(wrapped, expected);
}

#[test]
fn a_command_without_arguments_still_ends_with_the_program() {
    let sandboxed = Sandboxed {
        helper: PathBuf::from("miyu-sandbox"),
        spec: Spec::from_json("{}").expect("读得懂"),
        env: Vec::new(),
    };
    let (_, wrapped) = argv(&sandboxed, Path::new("true"), &[]).expect("包得成");
    assert_eq!(wrapped.last(), Some(&OsString::from("true")));
    assert_eq!(wrapped.len(), 5);
}
