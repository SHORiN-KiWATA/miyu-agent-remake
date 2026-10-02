use super::*;

const KEEP: Keep = Keep {
    found: Duration::from_secs(6 * 3600),
    no_preview: Duration::from_secs(15 * 60),
    unreachable: Duration::from_secs(45),
    entries: 3,
};

fn card() -> Card {
    Card {
        url: "https://example.com/".to_string(),
        title: "x".to_string(),
        description: String::new(),
        site: "example.com".to_string(),
        image: None,
        icon: None,
    }
}

#[test]
fn each_kind_is_kept_for_its_own_time() {
    let remember = Remember::new(KEEP);
    let start = Instant::now();
    remember.put("found".to_string(), Ok(card()), start);
    remember.put("no".to_string(), Err(Why::NoPreview), start);
    remember.put("down".to_string(), Err(Why::Unreachable), start);
    let at = |seconds: u64| start + Duration::from_secs(seconds);
    // 45 秒：网络抖了的过期了，别的还在
    assert_eq!(remember.get("down", at(44)), Some(Err(Why::Unreachable)));
    assert_eq!(remember.get("down", at(45)), None);
    // 15 分钟：做不出卡片的过期了
    assert_eq!(
        remember.get("no", at(15 * 60 - 1)),
        Some(Err(Why::NoPreview))
    );
    assert_eq!(remember.get("no", at(15 * 60)), None);
    // 6 小时：抓到了的过期了
    assert_eq!(remember.get("found", at(6 * 3600 - 1)), Some(Ok(card())));
    assert_eq!(remember.get("found", at(6 * 3600)), None);
    assert_eq!(remember.get("never", start), None);
}

#[test]
fn a_full_table_is_cleared_rather_than_grown() {
    let remember = Remember::new(KEEP);
    let now = Instant::now();
    for key in ["a", "b", "c"] {
        remember.put(key.to_string(), Err(Why::NoPreview), now);
    }
    // 记过的再记一遍不算多一条，不清空
    remember.put("a".to_string(), Ok(card()), now);
    assert_eq!(remember.get("b", now), Some(Err(Why::NoPreview)));
    assert_eq!(remember.get("a", now), Some(Ok(card())));
    // 第四条：满了，整个清空再记
    remember.put("d".to_string(), Err(Why::Unreachable), now);
    for key in ["a", "b", "c"] {
        assert_eq!(remember.get(key, now), None, "{key} 清掉了");
    }
    assert_eq!(remember.get("d", now), Some(Err(Why::Unreachable)));
}
