use super::*;

#[test]
fn the_platform_names_are_the_ones_written_in_json() {
    for platform in [
        Platform::Linux,
        Platform::Macos,
        Platform::Windows,
        Platform::Other,
    ] {
        let json = serde_json::to_string(&platform).expect("写得成");
        assert_eq!(json, format!("\"{}\"", platform.name()));
    }
}

#[test]
fn this_build_knows_its_platform() {
    let expected = match std::env::consts::OS {
        "linux" => Platform::Linux,
        "macos" => Platform::Macos,
        "windows" => Platform::Windows,
        _ => Platform::Other,
    };
    assert_eq!(Platform::current(), expected);
}

#[test]
fn the_probe_of_this_machine_is_one_line_of_version_one() {
    let here = Probe::new(vec!["landlock".into()]);
    assert_eq!(here.version, VERSION);
    assert_eq!(VERSION, 1);
    let json = serde_json::to_string(&here).expect("写得成");
    assert_eq!(
        json,
        format!(
            r#"{{"version":1,"platform":"{}","mechanisms":["landlock"]}}"#,
            Platform::current().name()
        )
    );
}

#[test]
fn mechanisms_are_joined_or_none() {
    assert_eq!(Probe::new(Vec::new()).mechanisms_text(), "none");
    let probe = Probe::new(vec!["landlock".into(), "seccomp".into()]);
    assert_eq!(probe.mechanisms_text(), "landlock,seccomp");
}

#[test]
fn each_failure_says_what_went_wrong() {
    let cases = [
        (
            ProbeError::Start(io::Error::new(io::ErrorKind::NotFound, "gone")),
            "cannot run helper: gone",
        ),
        (ProbeError::TimedOut, "helper timed out"),
        (
            ProbeError::Unreadable("version 2".into()),
            "helper output not understood: version 2",
        ),
    ];
    for (error, said) in cases {
        assert_eq!(error.to_string(), said);
    }
}
