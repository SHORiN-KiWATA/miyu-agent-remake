//! 伪终端测试的数据根（蓝图 `tui.md`「守着它的」）：临时的数据根和工作目录，上面跑着一份照剧本回话的核心；起界面交给
//! [`Tui`]。从 `mod.rs` 拆出来（2026-10-11：过了 500 行）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::channel;

use miyu_endpoint::config::Config;
use miyu_endpoint::{Core, run};
use miyu_ipc::{Dirs, open};
use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::env::{Env, Platform};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

use super::{Tui, WAIT, resources};

/// 一个用完就删的临时数据根和工作目录，上面跑着一份照剧本回话的核心。
pub struct Home {
    dir: PathBuf,
    /// 界面的工作目录：在数据根外面。
    pub work: PathBuf,
    /// 核心跑在这个线程的运行时里；丢掉 `Home` 时整个进程的测试线程都还在，核心跟着测试进程走。
    _core: std::thread::JoinHandle<()>,
}

/// 个人设置里记上「第一次引导走过了」：已经有 `[ui]` 的接在它下面（TOML 一张表只能写一次），没有的另起一张。
fn welcomed(settings: &str) -> String {
    match settings.find("[ui]\n") {
        Some(at) => {
            let (head, rest) = settings.split_at(at + "[ui]\n".len());
            format!("{head}welcomed = true\n{rest}")
        }
        None => format!("{settings}\n[ui]\nwelcomed = true\n"),
    }
}

impl Home {
    /// 起一份核心：管理员 alice，请求模型照 `script` 回，没有工具。
    pub fn new(script: Script) -> Home {
        Home::with_settings(script, "")
    }

    /// 同 [`Home::new`]，核心起来以前先写好 alice 的个人设置（`home/alice/settings.toml`，TOML）。都记着第一次引导走过了
    /// （`ui.welcomed`，核心 8-11 四补），不然一起来就进引导；要引导的用 [`Home::unwelcomed`]。
    pub fn with_settings(script: Script, settings: &str) -> Home {
        Home::build(script, &welcomed(settings), false)
    }

    /// 同 [`Home::with_settings`]，但没走过第一次引导：一起来就进引导（蓝图「第一次打开的引导」第 1 条）。
    pub fn unwelcomed(script: Script, settings: &str) -> Home {
        Home::build(script, settings, false)
    }

    /// 同 [`Home::with_settings`]，另外装上基础系统的工具（读写文件、跑命令这些）：她调了要确认的，核心来问。
    pub fn with_tools(script: Script, settings: &str) -> Home {
        Home::build(script, &welcomed(settings), true)
    }

    fn build(script: Script, settings: &str, tools: bool) -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-tui-{}-{n}", std::process::id()));
        let work = std::env::temp_dir().join(format!("miyu-tui-work-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&work).expect("建得了工作目录");
        let work = std::fs::canonicalize(&work).expect("在");
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(dir.clone().into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("临时目录里建得了骨架");
        if !settings.is_empty() {
            let file = dir.join("home/alice/settings.toml");
            std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
            std::fs::write(&file, settings).expect("写得进去");
        }
        let (ready, started) = channel();
        let core = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("起得来运行时");
            runtime.block_on(async move {
                let dirs = Dirs {
                    runtime_dir: None,
                    ..Dirs::current()
                };
                let opened = open(&root, &dirs).expect("起得来");
                let admin = AccountId::parse("alice").expect("账号合写法");
                // 配置照核心起来时那样读：登记的全部配置项（`miyu_core::settings`）接上出厂软件包清单声明的
                // （终端的 `tui.*` 这些，照发行包的样子：程序都算在），环境变量一个都不认。
                let mut shipped =
                    miyu_store::packages::Packages::shipped(&ResourceRoot::at(resources())).read();
                let config = Config::load(
                    &root,
                    &admin,
                    None,
                    miyu_core::settings::Packaged::as_released(&mut shipped).all(),
                    miyu_endpoint::config::Environment::of(&[]),
                );
                let core = Arc::new(
                    Core::new(
                        root,
                        ResourceRoot::at(resources()),
                        Arc::new(script),
                        if tools {
                            {
                                // 照出厂的那一层清单登记（核心 F-2 起：装了的内置包才登记工具）。
                                let shipped = ResourceRoot::at(resources());
                                let found =
                                    miyu_store::packages::Packages::shipped(&shipped).read();
                                miyu_core::tools(&shipped, &found).expect("工具登记得上")
                            }
                        } else {
                            Catalog::default()
                        },
                        None,
                        admin,
                        opened.token,
                    )
                    .with_config(config),
                );
                ready.send(()).expect("测试还在等");
                run(opened.listener, core).await;
            });
        });
        started.recv_timeout(WAIT).expect("核心起来了");
        Home {
            dir,
            work,
            _core: core,
        }
    }

    /// 核心的数据根。
    pub fn root(&self) -> &Path {
        &self.dir
    }

    /// 在 alice 的家目录里放一个人格（核心 P-1 上）：`persona.toml` 写 `toml`，人设写 `prompt`（`None` 不写）。
    pub fn persona(&self, id: &str, toml: &str, prompt: Option<&str>) {
        let dir = self.dir.join("home/alice/personas").join(id);
        std::fs::create_dir_all(dir.join("prompts")).expect("建得了");
        std::fs::write(dir.join("persona.toml"), toml).expect("写得进去");
        if let Some(prompt) = prompt {
            std::fs::write(dir.join("prompts/persona.md"), prompt).expect("写得进去");
        }
    }

    /// alice 的个人设置现在写着什么（没有的是空的）。
    pub fn settings(&self) -> String {
        std::fs::read_to_string(self.dir.join("home/alice/settings.toml")).unwrap_or_default()
    }

    /// 在伪终端里起界面，连这份核心；`lang` 是系统语言（`LANG`）。
    pub fn tui(&self, lang: &str) -> Tui {
        Tui::spawn(&self.dir, &self.work, lang, &[], &[])
    }

    /// 同 [`Home::tui`]，指定启动参数。
    pub fn tui_args(&self, lang: &str, args: &[&str]) -> Tui {
        Tui::spawn(&self.dir, &self.work, lang, &[], args)
    }

    /// 同 [`Home::tui_args`]，另外带几个环境变量。
    pub fn tui_args_with(&self, lang: &str, args: &[&str], env: &[(&str, &str)]) -> Tui {
        Tui::spawn(&self.dir, &self.work, lang, env, args)
    }

    /// 同 [`Home::tui`]，终端开 `cols` 列宽（看侧边栏要够 `layout.json` 的 `sidebar_from`）。
    pub fn tui_wide(&self, lang: &str, cols: u16) -> Tui {
        self.tui_wide_in(lang, cols, &self.work)
    }

    /// 同 [`Home::tui_wide`]，界面起在 `work` 这个目录。
    pub fn tui_wide_in(&self, lang: &str, cols: u16, work: &Path) -> Tui {
        Tui::program_sized(
            (&self.dir, work),
            lang,
            &[],
            &[],
            env!("CARGO_BIN_EXE_miyu-tui-demo"),
            cols,
        )
    }

    /// 同 [`Home::tui_wide`]，另外带几个环境变量。
    pub fn tui_wide_with(&self, lang: &str, cols: u16, env: &[(&str, &str)]) -> Tui {
        Tui::program_sized(
            (&self.dir, &self.work),
            lang,
            env,
            &[],
            env!("CARGO_BIN_EXE_miyu-tui-demo"),
            cols,
        )
    }

    /// 同 [`Home::tui`]，另外带几个环境变量。
    pub fn tui_with(&self, lang: &str, env: &[(&str, &str)]) -> Tui {
        Tui::spawn(&self.dir, &self.work, lang, env, &[])
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.work));
        drop(std::fs::remove_dir_all(&self.dir));
    }
}
