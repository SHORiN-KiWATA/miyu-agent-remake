//! 系统通知：认终端、弹不弹响不响（蓝图 `tui.md`「系统通知」）。

use std::collections::HashMap;

use super::{Event, Notifier, Plan, Route, plan};
use crate::config::Config;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name| map.get(name).cloned()
}

#[test]
fn the_route_follows_the_terminal() {
    let osc9 = Config::builtin().unwrap().notify.osc9_programs;
    let route = |pairs: &[(&str, &str)]| Route::detect(&env(pairs), &osc9);
    assert_eq!(route(&[("TERM", "xterm-kitty")]), Route::Kitty);
    assert_eq!(
        route(&[("TERM", "xterm-256color"), ("KITTY_WINDOW_ID", "3")]),
        Route::Kitty
    );
    assert_eq!(route(&[("TERM_PROGRAM", "iTerm.app")]), Route::Osc9);
    assert_eq!(route(&[("TERM_PROGRAM", "ghostty")]), Route::Osc9);
    assert_eq!(route(&[("TERM", "xterm-256color")]), Route::System);
    // tmux 不转 OSC，herdr 吞掉 OSC：交给系统。
    assert_eq!(
        route(&[
            ("TERM", "xterm-kitty"),
            ("TMUX", "/tmp/tmux-1000/default,1,0")
        ]),
        Route::System
    );
    assert_eq!(
        route(&[("TERM", "xterm-kitty"), ("HERDR_ENV", "1")]),
        Route::System
    );
}

#[test]
fn it_only_pops_up_when_away_and_kitty_decides_for_itself() {
    // 2026-09-30 项目主人：终端不在前台时才弹、才响；在 herdr 里由 herdr 来响。
    let look = Config::builtin().unwrap().notify;
    let pop = |route| Plan {
        route: Some(route),
        sound: true,
    };
    assert_eq!(
        plan(&look, Route::System, Some(false), false),
        pop(Route::System)
    );
    assert_eq!(
        plan(&look, Route::System, Some(true), false),
        Plan {
            route: None,
            sound: false
        },
        "在前台：不打扰"
    );
    assert_eq!(
        plan(&look, Route::Osc9, None, false).route,
        None,
        "一次都没报过：当在前台"
    );
    // kitty 自己判前台（o=unfocused），界面只判响不响。
    assert_eq!(
        plan(&look, Route::Kitty, Some(true), false),
        Plan {
            route: Some(Route::Kitty),
            sound: false
        }
    );
    assert!(plan(&look, Route::Kitty, Some(false), false).sound);
    assert!(
        !plan(&look, Route::System, Some(false), true).sound,
        "在 herdr 里不响"
    );
    let mut off = look.clone();
    off.enabled = false;
    assert_eq!(
        plan(&off, Route::Kitty, Some(false), false).route,
        None,
        "关掉整个不弹"
    );
    let mut quiet = look;
    quiet.sound = false;
    assert!(!plan(&quiet, Route::System, Some(false), false).sound);
}

#[test]
fn kitty_gets_its_sequence_in_the_outbox_with_what_happened() {
    let config = Config::builtin().unwrap();
    let mut n = Notifier::new(
        config.notify.clone(),
        config.text.notify.clone(),
        env(&[("TERM", "xterm-kitty")]),
        None,
    );
    n.tell(Event::Replied);
    let out = n.outbox();
    assert_eq!(out.len(), 1);
    assert!(out[0].starts_with("\x1b]99;"), "{:?}", out[0]);
    assert!(n.outbox().is_empty(), "取走就空了");
}

#[test]
fn herdr_keeps_desktop_notifications_and_owns_only_sound() {
    let look = Config::builtin().unwrap().notify;
    assert_eq!(
        plan(&look, Route::System, Some(false), true),
        Plan {
            route: Some(Route::System),
            sound: false
        }
    );
    for focus in [None, Some(true)] {
        assert_eq!(
            plan(&look, Route::System, focus, true),
            Plan {
                route: None,
                sound: false
            }
        );
    }
}
