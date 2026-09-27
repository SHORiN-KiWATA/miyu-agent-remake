//! 核心说「好了」的那一行（`docs/designs/12-进程形态与分发.md` 第二节「拉起时的握手」，施工 3-9 上）：
//! 核心起来以后往标准输出写一行，拉起它的头等着读，不轮询。

/// 核心写的那一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ready {
    /// 好了：在套接字上等连接了。
    Ready,
    /// 已经有一个核心在跑：这一个走了，头去连那一个。
    Running,
    /// 起不来：原因，照原样给人看。
    Failed(String),
}

impl Ready {
    /// 写成一行，带换行。原因里的换行换成空格：只有一行。
    pub fn line(&self) -> String {
        match self {
            Ready::Ready => "ready\n".to_string(),
            Ready::Running => "running\n".to_string(),
            Ready::Failed(reason) => format!("error {}\n", reason.replace(['\r', '\n'], " ")),
        }
    }

    /// 从一行读回来。读不懂的当起不来，原因就是这一行。
    pub fn parse(line: &str) -> Ready {
        let line = line.trim_end_matches(['\r', '\n']);
        match line {
            "ready" => Ready::Ready,
            "running" => Ready::Running,
            _ => match line.strip_prefix("error ") {
                Some(reason) => Ready::Failed(reason.to_string()),
                None => Ready::Failed(line.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests;
