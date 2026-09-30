//! 在进程里的核心上办一次 `miyu config`（施工 8-3）：人那一头照测试给的剧本回。

use miyu_cli::{ConfigPlan, Console, config_on};

use super::{Asked, Home, Tape, within};

impl Home {
    /// 在真的套接字上连上核心，照 `plan` 办一次 `miyu config`，人那一头是 `console`。
    pub async fn config(&self, plan: &ConfigPlan, console: &mut dyn Console) -> Asked {
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let tape = Tape::default();
        let (mut out, mut err) = (tape.pen(false), tape.pen(true));
        let code = within(
            "办完",
            config_on(connection, &token, plan, console, &mut out, &mut err),
        )
        .await;
        Asked {
            code,
            out: tape.text(|err| !err),
            err: tape.text(|err| err),
            screen: tape.text(|_| true),
        }
    }
}
