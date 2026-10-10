//! 扩展进程的测试共用的（施工 9-4 上、补）：测试用的扩展、装清单、造核心、等状态。`tests/extensions.rs`、
//! `tests/extension_stream.rs` 用；带系统账号的清单、会话目录、等轮数、等库里数得到，`tests/system_account.rs` 和装上的
//! 包当场换（施工 F-5 下）用。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::extensions::Timing;
use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::testkit::Script;
use miyu_store::log::read_events;
use miyu_tool::Catalog;

use super::{Client, Home, TOKEN};

/// 测试用的扩展，放在测试程序旁边、名字各用各的。用完删掉。
///
/// 放的是硬链接，不是拷贝（test-ext 补）：拷的时候开着写的句柄，同一个测试程序里别的测试这时拉起子进程，子进程在 exec 以前
/// 也开着它；核心接着拉起刚拷好的这个，Linux 回 `ETXTBSY`（Text file busy），扩展就成了 `cannot_start`。负载下 60 趟红 7 趟。
/// 硬链接不写字节，没有写的句柄。连不成的（不在一个文件系统上）才拷。
pub struct Program(PathBuf);

impl Program {
    pub fn new() -> Program {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let exe = std::env::current_exe().expect("找得到测试程序");
        let dir = std::fs::canonicalize(&exe)
            .expect("测试程序在")
            .parent()
            .expect("有上一级")
            .to_path_buf();
        let path = dir.join(format!(
            "miyu-test-ext-{}-{n}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        let built = env!("CARGO_BIN_EXE_miyu-test-extension");
        if std::fs::hard_link(built, &path).is_err() {
            std::fs::copy(built, &path).expect("拷得了");
        }
        Program(path)
    }

    /// 清单里写的程序名：不带 `.exe`。
    pub fn name(&self) -> String {
        self.0
            .file_stem()
            .expect("有名字")
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.0));
    }
}

/// 管理员（测试里是 alice）家目录里的一份 `process` 清单：程序 `program`，参数 `args`，`start` 照写。
pub fn install(home: &Home, id: &str, program: &str, start: &str, args: &[String]) {
    install_asking(home, id, program, start, args, &[]);
}

/// 同 [`install`]，清单声明要 `capabilities` 这几个扩展能力（施工 9-4 下上）；空的不写这一行。
pub fn install_asking(
    home: &Home,
    id: &str,
    program: &str,
    start: &str,
    args: &[String],
    capabilities: &[&str],
) {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    let asking = match capabilities {
        [] => String::new(),
        names => {
            let names: Vec<String> = names.iter().map(|name| format!("{name:?}")).collect();
            format!("capabilities = [{}]\n", names.join(", "))
        }
    };
    home.write(
        &format!("home/alice/packages/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Echo\", zh = \"回声\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"E\" }}\n\n[process]\nargs = [{}]\nstart = \"{start}\"\n{asking}",
            args.join(", ")
        ),
    );
}

/// 短的等法：等 300 毫秒再杀，退避从 20 毫秒起、最多 100 毫秒；跑满 60 秒才算稳。
pub fn quick() -> Timing {
    Timing {
        grace: Duration::from_millis(300),
        stable: Duration::from_secs(60),
        backoff: Duration::from_millis(20),
        longest: Duration::from_millis(100),
    }
}

/// 同 [`core`]，配置清单照 `miyu-core` 起来时那样拼进包的配置项（施工 9-4 下下：握手交包自己的配置）。
pub fn core_with_settings(home: &Home, timing: Timing) -> Arc<Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(super::default_resources());
    let alice = miyu_kernel::id::AccountId::parse("alice").expect("账号合写法");
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice);
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(timing)
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 一份核心：清单装好了再造（核心起来时读一次），照开关拉起开着的。
pub fn core(home: &Home, timing: Timing) -> Arc<Core> {
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(timing),
    );
    core.start_extensions();
    core
}

/// 记下的文件在哪；记的那一步的参数。
pub fn record(home: &Home, id: &str) -> (PathBuf, String) {
    let path = home.work.join(format!("record-{id}"));
    let step = format!("record:{}", path.display());
    (path, step)
}

pub fn steps(steps: &[&str]) -> Vec<String> {
    steps.iter().map(ToString::to_string).collect()
}

/// `extension.status` 里编号是 `id` 的那一个。
pub async fn status(client: &mut Client, id: &str) -> Value {
    let reply = client.call("s", "extension.status", json!({})).await;
    reply["result"]["extensions"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

/// 等到编号是 `id` 的那一个合 `wanted`：最多 60 秒。
pub async fn until_state(client: &mut Client, id: &str, wanted: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let one = status(client, id).await;
        if wanted(&one) {
            return one;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到：{one}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn stderr(home: &Home, id: &str) -> String {
    read(&home.root.state().join("logs").join(format!("{id}.stderr")))
}

pub async fn call(client: &mut Client, method: &str, id: &str) -> Value {
    client.call("c", method, json!({"package": id})).await
}

/// 包 `id` 的清单：程序 `program`、参数 `args`，`start`、`system_account` 照写。
pub fn install_serving(
    home: &Home,
    id: &str,
    program: &str,
    args: &[String],
    start: &str,
    system_account: bool,
) {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        &format!("home/alice/packages/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"B\" }}\n\n[process]\nargs = [{}]\nstart = \"{start}\"\nsystem_account = {system_account}\n",
            args.join(", ")
        ),
    );
}

/// 会话 `session` 在账号 `account` 名下的目录。
pub fn dir(home: &Home, account: &AccountId, session: &str) -> std::path::PathBuf {
    home.root
        .session_dir(account, &SessionId::parse(session).expect("会话编号合写法"))
}

/// 等到 `dir` 里的日志结束了 `n` 轮。
pub async fn until_turns_in(dir: &Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let ended = read_events(dir)
            .unwrap_or_default()
            .iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count();
        if ended >= n {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到第 {n} 轮");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 只读打开 SQLite 库 `db`，照 `sql`（带一个参数 `value`）数到大于 0 为止，最多 60 秒。库还没建、表还没有的当 0。
pub async fn until_counted(db: &Path, sql: &str, value: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let counted =
            rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .and_then(|db| db.query_row(sql, [value], |row| row.get::<_, i64>(0)))
                .unwrap_or(0);
        if counted > 0 {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} 里数不到：{sql}",
            db.display()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
