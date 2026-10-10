//! `miyu setup`（施工 8-11，`docs/blueprint/cli/setup.md`）：在进程里起核心（真目录裁出来的一份，供应商、本机的服务是假
//! 服务器），假终端照剧本回，在真的套接字上走一遍。三个平台一样跑：人那一头是照剧本回的 `Console`（`cli/login.md` 的先例）。
//!
//! 常用的几家、本机的服务、自定义列成一张表（施工 8-11 再补）；环境变量里的 key 只引用不复制；贴的 key 先试、通了存成密钥；
//! 试不通回到上一步；本机的服务不要 key；已经配好的只写 `models.chat`；核心看不到的变量说清是哪个；屏幕上从头到尾没有 key；
//! `miyu ask` 没模型时先走一遍。配置里一个池都没有的，一起写三个预设的池（施工 8-8 补）。自定义的、取不到模型列表的在
//! `setup_custom.rs`。

use serde_json::json;

use crate::support::Home;
use crate::support::onboarding::{FAKE, Typist, answer, deepseek_at, listing, plan};
use miyu_cli::Setup;
use miyu_http::testkit::{Reply, Server};

const WRONG: &str = "sk-FAKE-WRONG-KEY-0002";

/// 认证失败。
fn unauthorized() -> Reply {
    Reply::error(
        401,
        &[],
        r#"{"error":{"message":"Authentication Fails (no such user)"}}"#,
    )
}

/// 屏幕上「N 毫秒」的数换成 N：每次不一样。
fn steady(screen: &str) -> String {
    let mut out = String::new();
    for line in screen.split_inclusive('\n') {
        match line.split_once("，") {
            Some((head, tail)) if head.contains("通了") => {
                let rest = tail.split_once(" 毫秒").map_or(tail, |(_, rest)| rest);
                out.push_str(&format!("{head}，N 毫秒{rest}"));
            }
            _ => out.push_str(line),
        }
    }
    out
}

/// 屏幕上、配置里没有 key。
fn no_key_on(home: &Home, screen: &str) {
    assert!(!screen.contains("FAKE"), "屏幕上有 key：{screen}");
    assert!(
        !home.personal_settings().contains("FAKE"),
        "{}",
        home.personal_settings()
    );
}

#[tokio::test]
async fn a_key_in_the_environment_is_referenced_never_copied() {
    let server = Server::start(vec![
        listing(&["deepseek-v4-pro", "deepseek-flash", "a-tiny"]),
        answer(),
    ])
    .await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["1", ""], &[]);
    let here = ["OPENAI_API_KEY", "DEEPSEEK_API_KEY", "NOT_LOOKED_FOR"];
    let asked = home
        .setup(&plan(Setup::default(), &here), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        steady(&asked.screen),
        "· 核心未读到 OPENAI_API_KEY（核心启动后才设置）。核心空闲退出后重新运行 miyu setup，或选这一家贴 key。\n\
         选一家：\n\
         \x20 1  DeepSeek      已找到 key\n\
         \x20    OpenAI        用不了：目录里没有它的地址\n\
         \x20    Anthropic     用不了：目录里没有它的地址\n\
         \x20 2  opencode Zen\n\
         \x20 3  自定义\n\
         选一个编号：试一下 DeepSeek……\n\
         · 通了：试的 deepseek-flash，N 毫秒收到第一个字。\n\
         选主对话的模型：\n\
         \x20 1  deepseek-flash（推荐）\n\
         \x20 2  a-tiny\n\
         \x20 3  deepseek-v4-pro\n\
         选一个编号，直接回车用推荐的：写好了：models.chat = deepseek/deepseek-flash\n"
    );
    let config = home.personal_settings();
    assert!(
        config.contains("key = { env = \"DEEPSEEK_API_KEY\" }"),
        "只引用：{config}"
    );
    assert!(
        config.contains("chat = \"deepseek/deepseek-flash\""),
        "{config}"
    );
    assert_eq!(home.secrets(), "", "不复制进密钥文件");
    no_key_on(&home, &asked.screen);
    let bearer = format!("Bearer {FAKE}");
    assert_eq!(
        server.received()[1].header("authorization"),
        Some(bearer.as_str())
    );
}

#[tokio::test]
async fn a_pasted_key_is_tried_first_and_kept_only_once_it_works() {
    let server = Server::start(vec![
        unauthorized(),
        unauthorized(),
        listing(&["deepseek-flash", "deepseek-v4-pro"]),
        answer(),
    ])
    .await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["1", "2"], &[WRONG, FAKE]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        steady(&asked.screen),
        "选一家：\n\
         \x20 1  DeepSeek\n\
         \x20    OpenAI        用不了：目录里没有它的地址\n\
         \x20    Anthropic     用不了：目录里没有它的地址\n\
         \x20 2  opencode Zen\n\
         \x20 3  自定义\n\
         选一个编号：粘贴 deepseek 的 key（不显示）：试一下 DeepSeek……\n\
         不通（发请求）：认证失败：HTTP 401: Authentication Fails (no such user)\n\
         粘贴 deepseek 的 key（不显示）：试一下 DeepSeek……\n\
         · 通了：试的 deepseek-flash，N 毫秒收到第一个字。\n\
         · deepseek 的 key 存好了\n\
         选主对话的模型：\n\
         \x20 1  deepseek-flash（推荐）\n\
         \x20 2  deepseek-v4-pro\n\
         选一个编号，直接回车用推荐的：写好了：models.chat = deepseek/deepseek-v4-pro\n"
    );
    assert_eq!(typist.hidden, 2, "试不通回到贴 key");
    let secrets = home.secrets();
    assert!(
        secrets.contains(&format!("deepseek = \"{FAKE}\"\n")),
        "{secrets}"
    );
    assert!(!secrets.contains(WRONG), "错的那个没存过：{secrets}");
    let config = home.personal_settings();
    assert!(
        config.contains("key = { secret = \"deepseek\" }"),
        "{config}"
    );
    assert!(
        config.contains("chat = \"deepseek/deepseek-v4-pro\""),
        "{config}"
    );
    no_key_on(&home, &asked.screen);
}

/// 贴 key 那一步取消了（施工 8-5 补：`Ctrl+C`，或者空行 `Ctrl+D`），`Console::hidden` 报
/// [`std::io::ErrorKind::Interrupted`]：整个 `miyu setup` 照取消办，退出码 130，配置文件、密钥文件一个字都没写。真的
/// 终端里的 `Ctrl+C` 这一条用的是真的伪终端，在 `crates/miyu/tests/login_tty.rs`。
#[tokio::test]
async fn cancelling_the_key_paste_writes_nothing() {
    let server = Server::start(vec![]).await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["1"], &[]);
    typist.cancel_key = true;
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 130, "{}", asked.screen);
    assert!(asked.screen.ends_with("没存，取消了\n"), "{}", asked.screen);
    assert_eq!(home.personal_settings(), "", "配置一个字都没写");
    assert_eq!(home.secrets(), "", "密钥文件一个字都没写");
    assert!(server.received().is_empty(), "没发出去任何请求");
}

#[tokio::test]
async fn a_local_service_needs_no_key() {
    let server = Server::start(vec![
        listing(&["qwen3-8b"]),
        listing(&["qwen3-8b"]),
        answer(),
    ])
    .await;
    let profiles =
        json!({"lab": {"name": "Lab", "driver": "openai-chat", "base_url": server.base_url}});
    let home = Home::onboarding("", &[], profiles);
    let mut typist = Typist::at_terminal(&["3", ""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert!(
        asked.screen.contains(&format!(
            "  2  opencode Zen\n  3  Lab           本机 {}\n  4  自定义\n",
            server.base_url
        )),
        "{}",
        asked.screen
    );
    assert_eq!(typist.hidden, 0, "不要 key");
    let config = home.personal_settings();
    assert!(
        config.contains("[providers.lab]\nlocal = true\n"),
        "本机的服务不要 key：写 local = true 算配好了（施工 8-25）：{config}"
    );
    assert!(!config.contains("key"), "{config}");
    assert!(config.contains("chat = \"lab/qwen3-8b\""), "{config}");
    assert_eq!(server.received()[1].header("authorization"), None);
}

#[tokio::test]
async fn a_provider_already_set_up_only_gets_models_chat() {
    let server = Server::start(vec![listing(&["deepseek-flash"]), answer()]).await;
    let config = "[providers.ds]\ncatalog = \"deepseek\"\nkey = { env = \"DEEPSEEK_API_KEY\" }\nprice_multiplier = 0.5\n";
    let home = Home::onboarding(config, &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["1", ""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert!(
        asked.screen.contains("  1  DeepSeek      已配好\n"),
        "{}",
        asked.screen
    );
    let written = home.personal_settings();
    assert_eq!(
        home.system_config(),
        config,
        "系统配置一个字没动（施工 T-11 起写个人设置）"
    );
    assert!(
        written.contains("chat = \"ds/deepseek-flash\""),
        "{written}"
    );
}

#[tokio::test]
async fn nothing_picked_or_no_key_ends_with_1() {
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], json!({}));
    let mut typist = Typist::at_terminal(&[""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1);
    assert!(
        asked.screen.ends_with("选一个编号：没选\n"),
        "{}",
        asked.screen
    );

    let mut typist = Typist::at_terminal(&["9", "x"], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert!(
        asked.screen.contains(
            "选一个编号：9 不是列出的编号\n选一个编号：x 不是列出的编号\n选一个编号：没选\n"
        ),
        "{}",
        asked.screen
    );

    // opencode Zen：环境里没有它的 key，要贴（DeepSeek 找到了 key，选它就真的发出去了）。
    let mut typist = Typist::at_terminal(&["2"], &["  "]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1);
    assert!(
        asked.screen.ends_with("（不显示）：没收到 key\n"),
        "{}",
        asked.screen
    );
    assert_eq!(home.personal_settings(), "", "什么都没写");
}

#[tokio::test]
async fn ask_without_a_model_goes_through_setup_first_at_a_terminal() {
    let server = Server::start(vec![listing(&["deepseek-flash"]), answer()]).await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut piped = Typist::piping("");
    let (ready, asked) = home.ready(&plan(Setup::default(), &[]), &mut piped).await;
    assert_eq!(ready, Err(5));
    assert_eq!(asked.err, "没有可用的模型：还没配。运行 miyu setup。\n");
    assert_eq!(home.sessions().len(), 0, "不造会话");

    let mut typist = Typist::at_terminal(&["1", ""], &[]);
    let (ready, asked) = home.ready(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(ready, Ok(()), "{}", asked.screen);
    assert!(
        asked
            .screen
            .starts_with("还没有模型，先接上一个。\n选一家：\n"),
        "{}",
        asked.screen
    );
    assert!(
        home.personal_settings()
            .contains("chat = \"deepseek/deepseek-flash\"")
    );

    let mut untouched = Typist::at_terminal(&[], &[]);
    let (ready, asked) = home
        .ready(&plan(Setup::default(), &[]), &mut untouched)
        .await;
    assert_eq!(ready, Ok(()), "有了就不问：{}", asked.screen);
    assert_eq!(asked.screen, "");
    assert_eq!(untouched.lines, 0);
}

/// 三个预设的池（施工 8-8 补，`cli/setup.md` 第 10 条）：配置里一个池都没有的，和 `models.chat` 一起写进个人设置（施工 T-11），成员是空的、
/// 开关开着、不带说明；已经有池的（连同只写了开关的）不写。屏幕上照旧只说 `models.chat`。
#[tokio::test]
async fn three_preset_pools_go_in_only_when_there_are_no_pools() {
    let presets = "[pools.lite]\nmodels = []\nsubagent = true\n\n\
                   [pools.standard]\nmodels = []\nsubagent = true\n\n\
                   [pools.flagship]\nmodels = []\nsubagent = true\n";
    for (config, wanted) in [("", true), ("[pools.mine]\nsubagent = true\n", false)] {
        let server = Server::start(vec![listing(&["deepseek-flash"]), answer()]).await;
        let home = Home::onboarding(config, &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
        let mut typist = Typist::at_terminal(&["1", ""], &[]);
        let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
        assert_eq!(asked.code, 0, "{}", asked.screen);
        assert!(
            asked
                .screen
                .ends_with("写好了：models.chat = deepseek/deepseek-flash\n"),
            "{}",
            asked.screen
        );
        let written = home.personal_settings();
        assert_eq!(written.contains(presets), wanted, "{config:?}：{written}");
        assert_eq!(
            home.system_config(),
            config,
            "系统配置一个字没动（施工 T-11 起写个人设置）"
        );
        assert!(!written.contains("description"), "不带说明：{written}");
    }
}

/// 个人设置里已经有 `models.chat` 的（施工 T-11，网页的第一次引导先碰上的）：写进个人设置，换上的就是新选的，不被原来那一项
/// 盖住。原来写系统配置，屏幕上说写好了，会话照旧用个人设置里那一个。
#[tokio::test]
async fn a_chat_model_in_personal_settings_is_replaced() {
    let server = Server::start(vec![
        listing(&["deepseek-v4-pro", "deepseek-flash", "a-tiny"]),
        answer(),
    ])
    .await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let personal = home.root.path().join("home/admin/settings.toml");
    std::fs::create_dir_all(personal.parent().expect("有上一层")).expect("建得了目录");
    std::fs::write(&personal, "[models]\nchat = \"magpie/old-model\"\n").expect("写得进");
    let mut typist = Typist::at_terminal(&["1", ""], &[]);
    let asked = home
        .setup(&plan(Setup::default(), &["DEEPSEEK_API_KEY"]), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let written = home.personal_settings();
    assert!(
        written.contains("chat = \"deepseek/deepseek-flash\""),
        "换成新选的：{written}"
    );
    assert!(!written.contains("magpie/old-model"), "{written}");
    assert!(
        !home.system_config().contains("models"),
        "系统配置不碰：{}",
        home.system_config()
    );
}
