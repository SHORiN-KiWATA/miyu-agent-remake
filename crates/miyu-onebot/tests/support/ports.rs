//! 挑的空端口被别人先占了，换一组再来（偶发红的根因，2026-10-08 查的）：[`free_port`] 绑 `127.0.0.1:0` 拿到号就放掉，过一阵
//! 真的程序才去绑；机器负载高时这个号可能先被别的测试拿走（进程里的桥照端口 0 让系统挑、别的测试的 [`free_port`]），桥说端口
//! 被占、退出码 1。真的程序起桥的测试照 [`on_free_ports`] 跑：桥说的正是挑的那两个端口被占了，这一组作废、从头再来；别的原因
//! 退出照旧当失败。进程里起桥的给端口 0、让系统挑，碰不到这件事。

use miyu_onebot::serve::Failure;
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;

use super::resources;
use super::spawning::free_port;

/// 最多试几组端口。
pub const TRIES: usize = 5;

/// 桥说挑的端口被占了：这一组作废，换一组再来。
#[derive(Debug)]
pub struct Taken;

/// 挑两个空端口（NapCat 的、WebUI 的）跑 `attempt`；它交回 [`Taken`] 的，换一组从头再跑，最多 [`TRIES`] 次。
pub async fn on_free_ports<T>(attempt: impl AsyncFn(u16, u16) -> Result<T, Taken>) -> T {
    for _ in 0..TRIES {
        if let Ok(done) = attempt(free_port(), free_port()).await {
            return done;
        }
    }
    panic!("试了 {TRIES} 组端口，桥都说被占了");
}

/// 桥说的 `said`（标准错误）里有没有说 `listen`、`web` 被占了。三种语言的那两句都认：说哪种看配置和握手，没写 `ui.language`
/// 的照系统的语言说（测试里是英文）。
pub fn taken(said: &str, listen: u16, web: u16) -> bool {
    ["zh", "en", "ja"].into_iter().any(|language| {
        let texts = Texts::load(ResourceRoot::at(resources()), language).expect("读得出来");
        said.contains(&texts.failure(&Failure::PortInUse(listen)))
            || said.contains(&texts.failure(&Failure::WebPortInUse(web)))
    })
}
