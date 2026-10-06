use super::{Col, Nav, Org, Page, Search, orgs, shown_name};
use crate::settings::test_support::sample;

#[test]
fn the_org_column_appears_only_for_providers_with_prefixed_models() {
    let view = sample();
    let mut nav = Nav::default();
    assert_eq!(nav.cols(&view), [Col::Provider, Col::Model], "dev 没有前缀");
    nav.vertical(1, &view);
    assert_eq!(nav.cols(&view), [Col::Provider, Col::Org, Col::Model]);
    let relay = view.provider("relay").unwrap();
    assert_eq!(
        orgs(relay),
        vec![
            (Org::All, 3),
            (Org::Named("cline".into()), 1),
            (Org::Named("zhipu".into()), 1),
            (Org::Other, 1)
        ],
        "带前缀、不带的都有：末尾「其他」"
    );
}

#[test]
fn picking_an_org_filters_models_and_strips_the_prefix() {
    let view = sample();
    let mut nav = Nav::default();
    nav.vertical(1, &view);
    nav.sideways(true, &view);
    assert_eq!(nav.focus(&view), Col::Org);
    nav.vertical(2, &view);
    let models = nav.models(&view);
    assert_eq!(models.len(), 1);
    assert_eq!(shown_name(models[0], nav.org(&view).as_ref()), "glm-5v");
    nav.vertical(1, &view);
    assert_eq!(nav.models(&view)[0].name, "gpt-oss", "「其他」是不带前缀的");
}

#[test]
fn moving_past_the_last_column_turns_the_page_and_stops_at_the_ends() {
    let view = sample();
    let mut nav = Nav::default();
    nav.sideways(false, &view);
    assert_eq!((nav.page, nav.col), (Page::Providers, 0), "第一页最左停住");
    nav.sideways(true, &view);
    nav.sideways(true, &view);
    assert_eq!(
        (nav.page, nav.col),
        (Page::Defaults, 0),
        "最右一栏再往右进下一页"
    );
    nav.sideways(true, &view);
    assert_eq!((nav.page, nav.col), (Page::Pools, 0));
    nav.sideways(true, &view);
    nav.sideways(true, &view);
    assert_eq!((nav.page, nav.col), (Page::Pools, 1), "最后一页最右停住");
    nav.page_step(false);
    nav.sideways(false, &view);
    assert_eq!(nav.page, Page::Providers);
    assert_eq!(nav.col, 1, "从右边进来落在最后一栏");
}

#[test]
fn a_search_narrows_only_its_own_column_and_moving_resets_what_is_below() {
    let view = sample();
    let mut nav = Nav {
        search: Some(Search {
            col: Some(Col::Provider),
            text: "中转".into(),
            typing: false,
        }),
        ..Nav::default()
    };
    assert_eq!(nav.providers(&view).len(), 1);
    assert_eq!(nav.provider(&view).unwrap().id, "relay");
    nav.search = None;
    nav.vertical(1, &view);
    nav.col = 2;
    nav.vertical(2, &view);
    assert_eq!(nav.selected(Col::Model), 2);
    nav.col = 0;
    nav.vertical(-1, &view);
    assert_eq!(nav.selected(Col::Model), 0, "换了供应商，模型回到第一行");
    nav.page_step(true);
    nav.page_step(true);
    assert_eq!(nav.pool(&view).unwrap().name, "daily");
    assert_eq!(nav.len(Col::Member, &view), 2);
}
