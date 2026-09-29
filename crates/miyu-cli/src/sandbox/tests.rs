//! `miyu sandbox setup`、`remove` 怎么走（`docs/blueprint/sandbox/windows.md`「怎么走」第 2、3、6 条）：用替身当「机器」，
//! 看每一种情形它调了什么、交回哪一句；给人看的每一句，两种语言。

use std::cell::RefCell;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use clap::{Args, Command, FromArgMatches};
use miyu_sandbox::install::{Elevation, InstallError, Owner};

use super::flow::{Machine, child_args, run};
use super::{Said, Sandbox, Which};
use crate::language::Language;

/// 本人的 SID：写法对的一个。
const MY_SID: &str = "S-1-5-21-1-2-3-1001";
/// 参数给的另一个人的 SID。
const GIVEN_SID: &str = "S-1-5-21-9-8-7-1002";

/// 本人的数据根。
fn my_home() -> PathBuf {
    std::env::temp_dir().join("miyu-me")
}

/// 参数给的数据根：路径里带空格。
fn given_home() -> PathBuf {
    std::env::temp_dir().join("someone else").join(".miyu")
}

/// 替身调了什么。
#[derive(Debug, Clone, PartialEq)]
enum Call {
    Me,
    Elevated,
    Elevate(Vec<OsString>),
    Apply(Which, Owner),
    Report(Owner, InstallError),
    Reported(Owner),
    Clear(Owner),
}

/// 替身：照剧本回，记下调了什么。
struct Fake {
    elevated: Option<bool>,
    me: Result<Owner, InstallError>,
    elevate: RefCell<Option<Result<u32, Elevation>>>,
    apply: Result<(), InstallError>,
    reported: Option<InstallError>,
    calls: RefCell<Vec<Call>>,
}

impl Fake {
    /// 已经是管理员、本人是 [`my_home`]、干活都成。
    fn admin() -> Fake {
        Fake {
            elevated: Some(true),
            me: Owner::new(my_home(), MY_SID.into()),
            elevate: RefCell::new(None),
            apply: Ok(()),
            reported: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    /// 不是管理员，起提升过的自己交回 `outcome`。
    fn user(outcome: Result<u32, Elevation>) -> Fake {
        Fake {
            elevated: Some(false),
            elevate: RefCell::new(Some(outcome)),
            ..Fake::admin()
        }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }

    fn called(&self, call: Call) {
        self.calls.borrow_mut().push(call);
    }
}

impl Machine for Fake {
    fn elevated(&self) -> io::Result<bool> {
        self.called(Call::Elevated);
        self.elevated.ok_or_else(|| io::Error::other("no token"))
    }

    fn me(&self) -> Result<Owner, InstallError> {
        self.called(Call::Me);
        self.me.clone()
    }

    fn elevate(&self, args: &[OsString]) -> Result<u32, Elevation> {
        self.called(Call::Elevate(args.to_vec()));
        self.elevate.borrow_mut().take().expect("剧本里有这一步")
    }

    fn apply(&self, which: Which, owner: &Owner) -> Result<(), InstallError> {
        self.called(Call::Apply(which, owner.clone()));
        self.apply.clone()
    }

    fn report(&self, owner: &Owner, error: &InstallError) -> Result<(), InstallError> {
        self.called(Call::Report(owner.clone(), error.clone()));
        Ok(())
    }

    fn reported(&self, owner: &Owner) -> Result<Option<InstallError>, InstallError> {
        self.called(Call::Reported(owner.clone()));
        Ok(self.reported.clone())
    }

    fn clear(&self, owner: &Owner) -> Result<(), InstallError> {
        self.called(Call::Clear(owner.clone()));
        Ok(())
    }
}

/// 本人。
fn me() -> Owner {
    Owner::new(my_home(), MY_SID.into()).expect("写法对")
}

/// 参数给的那个人。
fn given() -> Owner {
    Owner::new(given_home(), GIVEN_SID.into()).expect("写法对")
}

/// 参数给的那两个。
fn given_args() -> Option<(PathBuf, String)> {
    Some((given_home(), GIVEN_SID.into()))
}

#[test]
fn an_administrator_does_it_for_themself_and_writes_no_report() {
    let fake = Fake::admin();
    let said = run(&fake, Which::Setup, None);
    assert!(matches!(said, Said::Done(Which::Setup)), "{said:?}");
    assert_eq!(
        fake.calls(),
        [Call::Me, Call::Elevated, Call::Apply(Which::Setup, me())]
    );
}

#[test]
fn the_given_data_root_and_sid_are_the_ones_used() {
    // 提升过的自己可能是另一个管理员账号：替谁装照参数给的数据根和 SID，不看自己（施工 5-8 审过时加）。
    let fake = Fake::admin();
    let said = run(&fake, Which::Setup, given_args());
    assert!(matches!(said, Said::Done(Which::Setup)), "{said:?}");
    assert_eq!(
        fake.calls(),
        [Call::Elevated, Call::Apply(Which::Setup, given())],
        "没问自己是谁"
    );
}

#[test]
fn the_elevated_one_that_fails_writes_the_reason_for_the_one_waiting() {
    let failure = InstallError::failed("create user", "Access is denied.");
    let fake = Fake {
        apply: Err(failure.clone()),
        ..Fake::admin()
    };
    let said = run(&fake, Which::Setup, given_args());
    assert!(
        matches!(said, Said::Failed(Which::Setup, ref error) if *error == failure),
        "{said:?}"
    );
    assert_eq!(
        fake.calls(),
        [
            Call::Elevated,
            Call::Apply(Which::Setup, given()),
            Call::Report(given(), failure),
        ]
    );
}

#[test]
fn an_administrator_without_the_arguments_writes_no_report_even_when_it_fails() {
    let failure = InstallError::Administrator;
    let fake = Fake {
        apply: Err(failure.clone()),
        ..Fake::admin()
    };
    let said = run(&fake, Which::Remove, None);
    assert!(
        matches!(said, Said::Failed(Which::Remove, ref error) if *error == failure),
        "{said:?}"
    );
    assert!(
        !fake
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Report(..))),
        "没人等它，不写原因"
    );
}

#[test]
fn someone_else_starts_an_elevated_self_with_the_owner_in_the_arguments() {
    let fake = Fake::user(Ok(0));
    let said = run(&fake, Which::Setup, None);
    assert!(matches!(said, Said::Done(Which::Setup)), "{said:?}");
    assert_eq!(
        fake.calls(),
        [
            Call::Me,
            Call::Elevated,
            Call::Clear(me()),
            Call::Elevate(vec![
                "sandbox".into(),
                "setup".into(),
                "--owner-home".into(),
                my_home().into(),
                "--owner-sid".into(),
                MY_SID.into(),
            ]),
        ],
        "先清旧原因，再起；没自己干"
    );
}

#[test]
fn removing_starts_it_with_remove() {
    let fake = Fake::user(Ok(0));
    let said = run(&fake, Which::Remove, None);
    assert!(matches!(said, Said::Done(Which::Remove)), "{said:?}");
    let started = fake
        .calls()
        .into_iter()
        .find_map(|call| match call {
            Call::Elevate(args) => Some(args),
            _ => None,
        })
        .expect("起了");
    assert_eq!(started[1], OsString::from("remove"));
}

#[test]
fn a_cancelled_elevation_needs_an_administrator() {
    let fake = Fake::user(Err(Elevation::Cancelled));
    let said = run(&fake, Which::Remove, None);
    assert!(matches!(said, Said::NeedsAdmin(Which::Remove)), "{said:?}");
}

#[test]
fn a_failed_elevated_self_says_what_it_reported_and_clears_it() {
    let reason = InstallError::NotDataRoot {
        path: "C:\\x".into(),
    };
    let fake = Fake {
        reported: Some(reason.clone()),
        ..Fake::user(Ok(1))
    };
    let said = run(&fake, Which::Setup, None);
    assert!(
        matches!(said, Said::Failed(Which::Setup, ref error) if *error == reason),
        "{said:?}"
    );
    let calls = fake.calls();
    let read = calls
        .iter()
        .position(|call| *call == Call::Reported(me()))
        .expect("读了");
    assert_eq!(calls.get(read + 1), Some(&Call::Clear(me())), "读了就清");
}

#[test]
fn a_failed_elevated_self_without_a_report_says_its_exit_code() {
    let fake = Fake::user(Ok(7));
    let said = run(&fake, Which::Setup, None);
    assert!(
        matches!(
            said,
            Said::Failed(Which::Setup, InstallError::Failed { ref step, ref detail })
                if step == "elevate" && detail == "exit code 7"
        ),
        "{said:?}"
    );
}

#[test]
fn an_elevation_that_cannot_start_says_why() {
    let fake = Fake::user(Err(Elevation::Failed(io::Error::other("no shell"))));
    let said = run(&fake, Which::Setup, None);
    assert!(
        matches!(
            said,
            Said::Failed(Which::Setup, InstallError::Failed { ref step, ref detail })
                if step == "elevate" && detail == "no shell"
        ),
        "{said:?}"
    );
}

#[test]
fn not_knowing_whether_it_is_an_administrator_is_a_failure() {
    let fake = Fake {
        elevated: None,
        ..Fake::admin()
    };
    let said = run(&fake, Which::Setup, None);
    assert!(
        matches!(said, Said::Failed(Which::Setup, InstallError::Failed { ref step, .. }) if step == "elevate"),
        "{said:?}"
    );
    assert!(
        !fake
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Apply(..) | Call::Elevate(_))),
        "什么都没干"
    );
}

#[test]
fn not_finding_myself_stops_before_anything() {
    let fake = Fake {
        me: Err(InstallError::failed("find owner", "no home")),
        ..Fake::admin()
    };
    let said = run(&fake, Which::Setup, None);
    assert!(
        matches!(said, Said::Failed(Which::Setup, InstallError::Failed { ref step, .. }) if step == "find owner"),
        "{said:?}"
    );
    assert_eq!(fake.calls(), [Call::Me]);
}

#[test]
fn bad_owner_arguments_stop_before_anything() {
    for (home, sid) in [
        (PathBuf::from("relative"), GIVEN_SID.to_string()),
        (given_home(), "S-1-5-21-1)(A;;FA;;;WD".to_string()),
    ] {
        let fake = Fake::admin();
        let said = run(&fake, Which::Setup, Some((home, sid)));
        assert!(
            matches!(said, Said::Failed(Which::Setup, InstallError::Failed { ref step, .. }) if step == "check owner"),
            "{said:?}"
        );
        assert!(fake.calls().is_empty(), "什么都没干");
    }
}

#[test]
fn the_arguments_for_the_elevated_self_read_back_as_the_same_owner() {
    for which in [Which::Setup, Which::Remove] {
        // 参数是给主程序的：`miyu sandbox <setup 或 remove> …`，照主程序那样读。
        let args = child_args(which, &given());
        let miyu = Command::new("miyu").subcommand(Sandbox::augment_args(Command::new("sandbox")));
        let matches = miyu
            .try_get_matches_from(std::iter::once(OsString::from("miyu")).chain(args))
            .expect("读得懂");
        let sandbox = matches.subcommand_matches("sandbox").expect("是 sandbox");
        let parsed = Sandbox::from_arg_matches(sandbox).expect("读得懂");
        let (read_which, owner) = parsed.parts();
        assert_eq!(read_which, which);
        assert_eq!(owner, given_args());
    }
}

#[test]
fn each_outcome_reads_in_both_languages() {
    let cases = [
        (
            Said::Done(Which::Setup),
            "沙盒用户建好了。",
            "The sandbox user is set up.",
        ),
        (
            Said::Done(Which::Remove),
            "沙盒用户撤掉了。",
            "The sandbox user is removed.",
        ),
        (
            Said::NotNeeded,
            "这个平台不用装沙盒。",
            "Nothing to set up on this platform.",
        ),
        (
            Said::NeedsAdmin(Which::Setup),
            "要管理员权限：用管理员身份跑 miyu sandbox setup。",
            "Administrator rights are needed: run miyu sandbox setup as administrator.",
        ),
        (
            Said::NeedsAdmin(Which::Remove),
            "要管理员权限：用管理员身份跑 miyu sandbox remove。",
            "Administrator rights are needed: run miyu sandbox remove as administrator.",
        ),
        (
            Said::Failed(Which::Setup, InstallError::Administrator),
            "已经有一个叫 miyu-sandbox 的管理员账号，不动它。",
            "An administrator account named miyu-sandbox already exists; leaving it alone.",
        ),
        (
            Said::Failed(
                Which::Remove,
                InstallError::NotDataRoot {
                    path: "C:\\x".into(),
                },
            ),
            "C:\\x 不是 Miyu 的数据根。",
            "C:\\x is not a Miyu data root.",
        ),
        (
            Said::Failed(
                Which::Setup,
                InstallError::failed("create user", "Access is denied."),
            ),
            "装沙盒失败：create user：Access is denied.",
            "Sandbox setup failed: create user: Access is denied.",
        ),
        (
            Said::Failed(
                Which::Remove,
                InstallError::failed("delete user", "busy\x1b[2J"),
            ),
            "卸沙盒失败：delete user：busy\u{FFFD}[2J",
            "Sandbox removal failed: delete user: busy\u{FFFD}[2J",
        ),
    ];
    for (said, chinese, english) in cases {
        assert_eq!(Language::Chinese.sandbox(&said), chinese, "{said:?}");
        assert_eq!(Language::English.sandbox(&said), english, "{said:?}");
    }
}

#[test]
fn only_done_and_not_needed_exit_zero() {
    assert_eq!(Said::Done(Which::Setup).code(), 0);
    assert_eq!(Said::NotNeeded.code(), 0);
    assert_eq!(Said::NeedsAdmin(Which::Setup).code(), 1);
    assert_eq!(
        Said::Failed(Which::Setup, InstallError::Administrator).code(),
        1
    );
}
