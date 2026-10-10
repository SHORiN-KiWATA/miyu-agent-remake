//! `miyu setup` 每一步都能用参数跳过（施工 8-11，`docs/blueprint/cli/setup.md`「怎么走」第 1、5 到 10 条）：三个都写的一句
//! 不问；不在终端里的从管道读 key、用推荐的模型；不在终端里又没写 `--provider` 的退出码 2、不连核心；`--provider` 不是目录里
//! 能用的退出码 2；不在终端里试不通退出码 1。三个平台一样跑。

use serde_json::json;

use crate::support::Home;
use crate::support::onboarding::{FAKE, Typist, answer, deepseek_at, listing, plan};
use miyu_cli::Setup;
use miyu_http::testkit::{Reply, Server};

fn skip(provider: Option<&str>, env: Option<&str>, model: Option<&str>) -> Setup {
    Setup {
        provider: provider.map(str::to_string),
        env: env.map(str::to_string),
        model: model.map(str::to_string),
    }
}

#[tokio::test]
async fn every_step_skipped_asks_nothing() {
    let server = Server::start(vec![
        listing(&["deepseek-flash", "deepseek-v4-pro"]),
        answer(),
    ])
    .await;
    let home = Home::onboarding("", &[("MY_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&[], &[]);
    let setup = skip(Some("deepseek"), Some("MY_KEY"), Some("deepseek-v4-pro"));
    let asked = home.setup(&plan(setup, &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        (typist.lines, typist.hidden, typist.all),
        (0, 0, 0),
        "一句不问"
    );
    let body: serde_json::Value = serde_json::from_slice(&server.received()[1].body).expect("JSON");
    assert_eq!(body["model"], "deepseek-v4-pro", "试写了的那个");
    let config = home.personal_settings();
    assert!(config.contains("key = { env = \"MY_KEY\" }"), "{config}");
    assert!(
        config.contains("chat = \"deepseek/deepseek-v4-pro\""),
        "{config}"
    );
    assert!(
        asked
            .screen
            .ends_with("写好了：models.chat = deepseek/deepseek-v4-pro\n"),
        "{}",
        asked.screen
    );
}

#[tokio::test]
async fn outside_a_terminal_the_key_comes_from_the_pipe_and_the_model_is_the_recommended_one() {
    let server = Server::start(vec![listing(&["a-tiny", "deepseek-flash"]), answer()]).await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::piping(&format!("{FAKE}\n"));
    let asked = home
        .setup(&plan(skip(Some("deepseek"), None, None), &[]), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!((typist.lines, typist.hidden, typist.all), (0, 0, 1));
    assert!(home.secrets().contains(&format!("deepseek = \"{FAKE}\"")));
    assert!(
        home.personal_settings()
            .contains("chat = \"deepseek/deepseek-flash\"")
    );
    assert!(!asked.screen.contains("FAKE"), "{}", asked.screen);
}

#[tokio::test]
async fn a_found_key_is_used_for_the_named_provider() {
    let server = Server::start(vec![listing(&["deepseek-flash"]), answer()]).await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::piping("");
    let asked = home
        .setup(&plan(skip(Some("deepseek"), None, None), &[]), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(typist.all, 0, "不读管道");
    assert!(
        home.personal_settings()
            .contains("key = { env = \"DEEPSEEK_API_KEY\" }")
    );
    assert_eq!(home.secrets(), "");
}

#[tokio::test]
async fn outside_a_terminal_without_provider_is_misuse() {
    let home = Home::onboarding("", &[], json!({}));
    let mut typist = Typist::piping(FAKE);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 2);
    assert_eq!(
        asked.screen,
        "要在终端里选，或者写 miyu setup --provider <编号>\n"
    );
    assert_eq!(typist.all, 0);
}

#[tokio::test]
async fn an_unusable_or_unknown_provider_is_misuse() {
    let home = Home::onboarding("", &[], json!({}));
    for id in ["anthropic", "nope", "deep"] {
        let mut typist = Typist::piping(FAKE);
        let asked = home
            .setup(&plan(skip(Some(id), None, None), &[]), &mut typist)
            .await;
        assert_eq!(asked.code, 2, "{id}：{}", asked.screen);
        assert_eq!(asked.screen, format!("目录里没有能用的 {id}\n"));
    }
}

#[tokio::test]
async fn outside_a_terminal_a_failed_try_is_1_and_nothing_is_written() {
    let unauthorized = || Reply::error(401, &[], r#"{"error":{"message":"bad key"}}"#);
    let server = Server::start(vec![unauthorized(), unauthorized()]).await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::piping(FAKE);
    let asked = home
        .setup(&plan(skip(Some("deepseek"), None, None), &[]), &mut typist)
        .await;
    assert_eq!(asked.code, 1, "{}", asked.screen);
    assert!(
        asked.screen.contains("不通（发请求）：认证失败"),
        "{}",
        asked.screen
    );
    assert_eq!(home.secrets(), "", "试不通的 key 不存");
    assert_eq!(home.personal_settings(), "");
}
