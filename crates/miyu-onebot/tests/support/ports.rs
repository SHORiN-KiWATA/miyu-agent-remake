//! 挑的空端口被别人先占了，换一个再来（偶发红的根因，2026-10-08 查的）：[`free_port`] 绑 `127.0.0.1:0` 拿到号就放掉，过一阵
//! 真的程序才去绑；机器负载高时这个号可能先被别的测试拿走（进程里的桥照端口 0 让系统挑、别的测试的 [`free_port`]），桥说端口
//! 被占、退出码 1。真的程序起桥的测试照 [`on_free_port`] 跑：桥说的正是挑的那个端口被占了，作废、从头再来；别的原因退出照旧
//! 当失败。进程里起桥的给端口 0、让系统挑，碰不到这件事。施工 O-28 下起桥只开 NapCat 的一个端口（原来还有 WebUI 的）。

use miyu_onebot::serve::Failure;
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;

use super::resources;
use super::spawning::free_port;

/// 最多试几个端口。
pub const TRIES: usize = 5;

/// 桥说挑的端口被占了：作废，换一个再来。
#[derive(Debug)]
pub struct Taken;

/// 挑一个空端口（NapCat 的）跑 `attempt`；它交回 [`Taken`] 的，换一个从头再跑，最多 [`TRIES`] 次。
pub async fn on_free_port<T>(attempt: impl AsyncFn(u16) -> Result<T, Taken>) -> T {
    for _ in 0..TRIES {
        if let Ok(done) = attempt(free_port()).await {
            return done;
        }
    }
    panic!("试了 {TRIES} 个端口，桥都说被占了");
}

/// 桥说的 `said`（标准错误）里有没有说 `listen` 被占了。三种语言的那一句都认：照握手回的语言说（施工 O-20），看测试当的
/// 核心回的是哪种。
pub fn taken(said: &str, listen: u16) -> bool {
    ["zh", "en", "ja"].into_iter().any(|language| {
        let texts = Texts::load(ResourceRoot::at(resources()), language).expect("读得出来");
        said.contains(&texts.failure(&Failure::PortInUse(listen)))
    })
}
