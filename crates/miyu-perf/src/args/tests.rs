//! 参数：路径必写，数有默认，写错的说哪里不对。

use super::*;

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

const PATHS_GIVEN: &str = "--miyu /m --resources /r --design /d --work /w --out /o";

#[test]
fn defaults_fill_the_numbers() {
    let args = parse(&words(PATHS_GIVEN)).unwrap();
    assert_eq!(args.miyu, PathBuf::from("/m"));
    assert_eq!(args.out, PathBuf::from("/o"));
    assert_eq!(
        (args.runs, args.sessions, args.events, args.tail),
        (10, 10, 10_000, 30)
    );
    assert_eq!(
        (args.reloads, args.appends, args.reply_bytes),
        (5, 2000, 3500)
    );
    assert_eq!(args.idle_wait, Duration::from_secs(200));
}

#[test]
fn numbers_can_be_given() {
    let args = parse(&words(&format!(
        "{PATHS_GIVEN} --runs 3 --sessions 4 --events 500 --tail 6 --reloads 2 --appends 50 --reply-bytes 100 --idle-wait 1"
    )))
    .unwrap();
    assert_eq!(
        (args.runs, args.sessions, args.events, args.tail),
        (3, 4, 500, 6)
    );
    assert_eq!((args.reloads, args.appends, args.reply_bytes), (2, 50, 100));
    assert_eq!(args.idle_wait, Duration::from_secs(1));
}

#[test]
fn rendering_needs_only_the_design_and_the_output() {
    let args = parse(&words("--render /o/x.json --design /d --out /o")).unwrap();
    assert_eq!(args.render, Some(PathBuf::from("/o/x.json")));
    let error = parse(&words("--render /o/x.json --out /o")).unwrap_err();
    assert!(error.starts_with("缺 --design\n"), "{error}");
    let error = parse(&words("--design /d --out /o")).unwrap_err();
    assert!(error.starts_with("缺 --miyu\n"), "{error}");
}

#[test]
fn mistakes_say_what_is_wrong() {
    let error = parse(&words("--miyu /m --resources /r --design /d --work /w")).unwrap_err();
    assert!(error.starts_with("缺 --out\n"), "{error}");
    let error = parse(&words(&format!("{PATHS_GIVEN} --runs -1"))).unwrap_err();
    assert!(
        error.starts_with("--runs 要一个非负整数，收到「-1」"),
        "{error}"
    );
    let error = parse(&words(&format!("{PATHS_GIVEN} --fast 1"))).unwrap_err();
    assert!(error.starts_with("不认识 --fast"), "{error}");
    let error = parse(&words(&format!("{PATHS_GIVEN} --runs"))).unwrap_err();
    assert!(error.starts_with("--runs 后面要跟一个值"), "{error}");
}
