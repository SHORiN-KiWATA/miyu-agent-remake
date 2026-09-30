//! 换了名字的一件工具（施工 7-5 再补）：规格照原来那一件，只换名字；报路径、执行都交给它。拿它造改名以前的核心的目录：
//! 那时这一件叫旧名字，造出来的会话快照里冻着旧名字。

use std::sync::Arc;

use crate::{Call, Progress, Running, Spec, Target, Tool};

/// 叫 `name` 的 `tool`。
pub struct Renamed {
    spec: Spec,
    tool: Arc<dyn Tool>,
}

impl Renamed {
    /// `tool` 换成叫 `name`：说明、参数格式、访问类别都照它的。
    pub fn new(tool: Arc<dyn Tool>, name: &str) -> Arc<Renamed> {
        let spec = Spec {
            name: name.to_string(),
            ..tool.spec().clone()
        };
        Arc::new(Renamed { spec, tool })
    }
}

impl Tool for Renamed {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        self.tool.targets(call)
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        self.tool.run(call, progress)
    }
}
