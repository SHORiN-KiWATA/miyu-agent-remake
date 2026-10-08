//! 冷却的测试（`docs/blueprint/models.md`「守着它的」`cooldown/tests.rs`，施工 8-9）：翻倍、封顶、供应商说的更长、成功清零、
//! 到期先试、认证失败停整家供应商（施工 8-25），只有三类记冷却，规矩照配置。

use std::time::Duration;

use miyu_config::Values;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::time::Timestamp;

use super::*;
use crate::test_support::resolved;

/// 2026-10-01 08:00:00 再过 `secs` 秒。
fn at(secs: i64) -> Timestamp {
    let start = Timestamp::parse("2026-10-01T08:00:00.000Z").expect("合写法");
    Timestamp::from_unix_millis(start.unix_millis() + secs * 1000).expect("在范围里")
}

/// 照默认的规矩记一次失败，交回冷却多少秒。
fn fail(
    table: &mut Cooldowns,
    model: &str,
    class: &ErrorClass,
    said: Option<u64>,
    now: Timestamp,
) -> Option<u64> {
    table
        .fail(
            &Candidate::new("p", model),
            class,
            said,
            &Rules::default(),
            now,
        )
        .map(|recorded| recorded.for_ms / 1000)
}

#[test]
fn each_failure_in_a_row_doubles_up_to_the_cap() {
    let mut table = Cooldowns::default();
    let waits: Vec<Option<u64>> = (0..7)
        .map(|n| fail(&mut table, "m", &ErrorClass::RateLimited, None, at(n)))
        .collect();
    let minutes = |m: u64| Some(m * 60);
    assert_eq!(
        waits,
        [
            Some(30),
            minutes(1),
            minutes(2),
            minutes(4),
            minutes(8),
            minutes(10),
            minutes(10)
        ],
        "30 秒起，翻倍，封顶 10 分钟"
    );
    let mut table = Cooldowns::default();
    for n in 0..200 {
        fail(&mut table, "m", &ErrorClass::Retryable, None, at(n));
    }
    assert_eq!(
        fail(&mut table, "m", &ErrorClass::Retryable, None, at(300)),
        Some(300),
        "连着失败很多次也不溢出，停在上限"
    );
}

#[test]
fn a_longer_wait_said_by_the_provider_wins_but_not_past_the_cap() {
    let class = ErrorClass::RateLimited;
    let mut table = Cooldowns::default();
    assert_eq!(fail(&mut table, "a", &class, Some(90_000), at(0)), Some(90));
    assert_eq!(
        fail(&mut table, "b", &class, Some(1_200_000), at(0)),
        Some(600)
    );
    assert_eq!(
        fail(&mut table, "c", &class, Some(5_000), at(0)),
        Some(30),
        "说得短的照算的"
    );
}

#[test]
fn a_unit_is_usable_again_when_it_expires_but_its_count_stays() {
    let mut table = Cooldowns::default();
    let unit = Candidate::new("p", "m");
    fail(&mut table, "m", &ErrorClass::RateLimited, None, at(0));
    assert_eq!(
        table.cooling(&unit, at(29)),
        Some(Cooling {
            until: at(30),
            class: ErrorClass::RateLimited
        })
    );
    assert_eq!(table.cooling(&unit, at(30)), None, "到点了就能用，先试它");
    assert_eq!(
        fail(&mut table, "m", &ErrorClass::RateLimited, None, at(31)),
        Some(60),
        "再失败照第 2 次算"
    );
}

#[test]
fn a_success_clears_the_count() {
    let mut table = Cooldowns::default();
    let unit = Candidate::new("p", "m");
    for n in 0..3 {
        fail(&mut table, "m", &ErrorClass::RateLimited, None, at(n));
    }
    table.succeed(&unit);
    assert_eq!(table.cooling(&unit, at(3)), None, "成了的不在冷却");
    assert_eq!(
        fail(&mut table, "m", &ErrorClass::RateLimited, None, at(4)),
        Some(30),
        "从第 1 次算"
    );
}

#[test]
fn an_auth_failure_stops_the_whole_provider() {
    let mut table = Cooldowns::default();
    assert_eq!(
        fail(&mut table, "m", &ErrorClass::Auth, None, at(0)),
        Some(600)
    );
    let cooling = Some(Cooling {
        until: at(600),
        class: ErrorClass::Auth,
    });
    assert_eq!(
        table.cooling(&Candidate::new("p", "other"), at(1)),
        cooling,
        "这一家的别的模型也停"
    );
    assert_eq!(table.provider_cooling("p", at(1)), cooling);
    assert_eq!(
        table.cooling(&Candidate::new("q", "m"), at(1)),
        None,
        "别的一家不停"
    );
    // 这一家的一个模型成了：整家的次数也清零。
    table.succeed(&Candidate::new("p", "other"));
    assert_eq!(table.provider_cooling("p", at(2)), None);
}

#[test]
fn the_later_of_the_provider_and_the_model_counts() {
    let mut table = Cooldowns::default();
    let unit = Candidate::new("p", "m");
    fail(&mut table, "m", &ErrorClass::Retryable, None, at(0));
    fail(&mut table, "x", &ErrorClass::Auth, None, at(0));
    assert_eq!(
        table.cooling(&unit, at(1)),
        Some(Cooling {
            until: at(600),
            class: ErrorClass::Auth
        })
    );
}

#[test]
fn only_three_classes_cool_anything() {
    let mut table = Cooldowns::default();
    for class in [
        ErrorClass::ContextTooLong,
        ErrorClass::ContentPolicy,
        ErrorClass::Unclassified,
        ErrorClass::BadStream,
        ErrorClass::EmptyReply,
        ErrorClass::NoModel,
        ErrorClass::Cooling,
    ] {
        assert_eq!(
            fail(&mut table, "m", &class, Some(1000), at(0)),
            None,
            "{class:?}"
        );
    }
    assert_eq!(table.cooling(&Candidate::new("p", "m"), at(0)), None);
    assert_eq!(
        fail(&mut table, "m", &ErrorClass::Retryable, None, at(0)),
        Some(10)
    );
}

#[test]
fn the_rules_come_from_the_settings() {
    assert_eq!(
        Rules::default(),
        Rules::from_values(&Values::default()),
        "不写照默认"
    );
    let rules = Rules::from_values(&resolved(
        "[models.cooldown.rate_limited]\nbase = \"5s\"\n\n[models.cooldown.auth]\nbase = \"1h\"\nmax = \"20m\"\n",
    ).values());
    assert_eq!(
        rules.rate_limited,
        Rule {
            base: Duration::from_secs(5),
            max: Duration::from_secs(600)
        }
    );
    assert_eq!(rules.retryable, Rules::default().retryable);
    let mut table = Cooldowns::default();
    let recorded = table
        .fail(
            &Candidate::new("p", "m"),
            &ErrorClass::Auth,
            None,
            &rules,
            at(0),
        )
        .expect("记了");
    assert_eq!(recorded.for_ms, 1_200_000, "base 比 max 大的照 max");
    assert_eq!(recorded.failures, 1);
    assert_eq!(recorded.until, at(1200));
}
