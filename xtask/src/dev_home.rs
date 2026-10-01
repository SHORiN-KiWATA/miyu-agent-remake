//! `cargo xtask dev-home <目录>`（`docs/blueprint/models.md`「怎么走」第十条第 2 条，施工 8-6）：开发时真模型自测，照三个
//! 环境变量造一个带配置的数据根，之后照平常 `MIYU_HOME=<目录> miyu ask …`。
//!
//! - `MIYU_DEV_BASE_URL`（地址，必设）、`MIYU_DEV_MODEL`（模型名，必设）、`MIYU_DEV_WINDOW`（窗口，可以不设）。这三个名字只在
//!   这里，程序里没有了。地址、key 都不进仓库，只在命令里。
//! - 先建骨架（照核心的写法，`store.md`「认得出自己的数据根才动它」），再写 `system/config.toml`：一家 `dev`
//!   （`openai-chat`，`catalog = "deepseek"` 照 DeepSeek 的档案配开关，key 照 `{ env = "DEEPSEEK_API_KEY" }` 取），
//!   `models.chat = "dev/<模型>"`，设了窗口的写进这个模型的 `window`。
//! - 已经有 `system/config.toml` 的不盖：人改过的配置不替人扔掉（「施工时定的」8-6）。
//!
//! 这个文件也被 `crates/miyu/tests/dev_home.rs` 原样编进去（`#[path]`），测试都在那里：测的就是这里造的数据根，核心认得出、
//! 照它连得上假服务器。

use std::io::Write;
use std::path::{Path, PathBuf};

use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 三个环境变量的名字。
pub const BASE_URL: &str = "MIYU_DEV_BASE_URL";
/// 模型名。
pub const MODEL: &str = "MIYU_DEV_MODEL";
/// 窗口。
pub const WINDOW: &str = "MIYU_DEV_WINDOW";

/// 配置里 key 照哪个环境变量取：拉起核心的那个终端里设它。
pub const KEY_ENV: &str = "DEEPSEEK_API_KEY";

/// 照环境变量读好的三样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vars {
    /// 地址。
    pub base_url: String,
    /// 模型名。
    pub model: String,
    /// 窗口：没设的没有。
    pub window: Option<u64>,
}

impl Vars {
    /// 照 `get`（名字到值）读：去掉前后空白，空的当没设。
    ///
    /// # Errors
    ///
    /// 地址、模型名没设；地址不是 `http://`、`https://` 开头；模型名超过 128 字节、有控制字符；窗口不是正整数。
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Result<Vars, String> {
        let set = |name: &str| {
            get(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let base_url = set(BASE_URL).ok_or_else(|| format!("设 {BASE_URL}：模型服务的地址"))?;
        let lower = base_url.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(format!("{BASE_URL} 要是 http:// 或 https:// 开头的地址"));
        }
        let model = set(MODEL).ok_or_else(|| format!("设 {MODEL}：模型名"))?;
        if model.len() > 128 || model.chars().any(char::is_control) {
            return Err(format!("{MODEL} 要是 1 到 128 字节、没有控制字符的模型名"));
        }
        let window = match set(WINDOW) {
            None => None,
            Some(text) => Some(
                text.parse::<u64>()
                    .ok()
                    .filter(|window| (1..=100_000_000).contains(window))
                    .ok_or_else(|| format!("{WINDOW} 要是 1 到 100000000 的整数"))?,
            ),
        };
        Ok(Vars {
            base_url,
            model,
            window,
        })
    }

    /// 写进 `system/config.toml` 的字。
    pub fn config(&self) -> String {
        let model = quoted(&self.model);
        let mut text = format!(
            "#:schema ../state/config/config.schema.json\n\
             # cargo xtask dev-home 造的，开发自测用。key 照拉起核心的那个终端里的 {KEY_ENV} 取。\n\
             \n\
             [providers.dev]\n\
             driver = \"openai-chat\"\n\
             base_url = {}\n\
             catalog = \"deepseek\"\n\
             keys = [{{ env = \"{KEY_ENV}\" }}]\n",
            quoted(&self.base_url)
        );
        if let Some(window) = self.window {
            text.push_str(&format!(
                "\n[providers.dev.models.{model}]\nwindow = {window}\n"
            ));
        }
        text.push_str(&format!(
            "\n[models]\nchat = {}\n",
            quoted(&format!("dev/{}", self.model))
        ));
        text
    }
}

/// TOML 的基本字符串：两头双引号，引号、反斜杠转义（控制字符读的时候已经不收）。
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 在 `dir`（绝对路径）造一个数据根，写好系统配置，交回配置文件在哪。
///
/// # Errors
///
/// 不是绝对路径；目录里有别的东西、认不出是 Miyu 的数据根；建不了、写不进；已经有系统配置。
pub fn make(dir: &Path, vars: &Vars) -> Result<PathBuf, String> {
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.as_os_str().to_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .map_err(|error| error.to_string())?;
    root.prepare().map_err(|error| error.to_string())?;
    let path = root.system().join("config.toml");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => format!(
                "{} 已经有了，不盖：要重造就换一个目录，或者用 miyu config 改",
                path.display()
            ),
            _ => format!("写不了 {}：{error}", path.display()),
        })?;
    file.write_all(vars.config().as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("写不了 {}：{error}", path.display()))?;
    Ok(path)
}

/// 命令行：`dev-home <目录>`。相对路径照当前目录接上。说清造在哪、接着怎么用。
pub fn run(dir: Option<&str>) -> std::process::ExitCode {
    let Some(dir) = dir else {
        eprintln!(
            "用法：cargo xtask dev-home <目录>（先设 {BASE_URL}、{MODEL}，{WINDOW} 可以不设）"
        );
        return std::process::ExitCode::from(2);
    };
    let dir = match std::env::current_dir() {
        Ok(here) => here.join(dir),
        Err(error) => {
            eprintln!("不知道当前目录：{error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let made = Vars::read(|name| std::env::var(name).ok()).and_then(|vars| make(&dir, &vars));
    match made {
        Ok(path) => {
            println!("写好了 {}", path.display());
            println!(
                "接着在设了 {KEY_ENV} 的终端里：MIYU_HOME={} miyu ask …",
                dir.display()
            );
            std::process::ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            std::process::ExitCode::FAILURE
        }
    }
}
