//! 拒绝时给人看的话（`docs/designs/04-核心协议.md` 第六节第 4 条）：一个原因码一句中文、一句英文。施工 W-8 从
//! `refusal.rs` 挪出来（那一份过了 500 行）。

/// 原因码 `reason` 的中文、英文。不认识的说「被拒绝了」。
pub(super) fn of(reason: &str) -> (&'static str, &'static str) {
    match reason {
        "parse_error" => ("读不懂这条消息。", "The message could not be read."),
        "invalid_request" => ("这不是一条请求。", "This is not a request."),
        "unknown_method" => ("没有这个方法。", "There is no such method."),
        "bad_params" => ("参数不对。", "The parameters are not right."),
        "internal_error" => (
            "核心出了问题，详情在运行日志里。",
            "The core ran into a problem; the runtime log has the details.",
        ),
        "hello_first" => (
            "连上以后要先打招呼（hello）。",
            "Say hello first after connecting.",
        ),
        "protocol_mismatch" => (
            "头和核心的协议版本对不上，请把它们升级到同一个版本。",
            "The head and the core speak different protocol versions; upgrade them to the same release.",
        ),
        "bad_token" => ("本机令牌不对。", "The local token is wrong."),
        "unknown_persona" => ("没有这个人格。", "There is no such persona."),
        "persona_invalid" => (
            "这个人格的文件写错了，详情在 data.problem 里。",
            "This persona's files have a mistake; data.problem says where.",
        ),
        "session_not_found" => ("没有这个会话。", "There is no such session."),
        "no_system_account" => (
            "这个场所的会话要归系统账号，还没有装好系统账号。",
            "This venue's session belongs to a system account, which is not set up yet.",
        ),
        "venue_session" => (
            "这是通讯平台的场所会话，本机的头不能直接说话。",
            "This is a chat platform venue session; local heads cannot talk in it directly.",
        ),
        "unknown_command" => ("没有这个命令。", "There is no such command."),
        "command_not_allowed" => (
            "只有主人和管理的人能用命令。",
            "Only the owner and managers can use commands.",
        ),
        "session_stopped" => (
            "这个会话停了，详情在运行日志里；再发一次会重新载入。",
            "This session has stopped; the runtime log has the details. Sending again reloads it.",
        ),
        "session_broken" => (
            "这个会话载入不了：它的日志或者策略快照坏了。",
            "This session cannot be loaded: its log or policy snapshot is broken.",
        ),
        "empty_message" => ("消息是空的。", "The message is empty."),
        "dir_too_wide" => (
            "加进来的目录太宽：家目录、根目录、Miyu 的数据根不能整个放行。",
            "An added directory is too wide: the home directory, the root and Miyu's data root cannot be opened up whole.",
        ),
        "attachment_unreadable" => (
            "读不了这个文件：没有、不是普通文件，或者没有权限。",
            "This file cannot be read: it is missing, not a regular file, or not permitted.",
        ),
        "attachment_too_big" => (
            "附件太大：一个最多 20 MiB，图片最多 5 MiB、每边最多 8000 像素。",
            "The attachment is too big: at most 20 MiB, and an image at most 5 MiB and 8000 pixels a side.",
        ),
        "attachment_in_data_root" => (
            "Miyu 的数据根里的文件不能当附件。",
            "Files in Miyu's data root cannot be attached.",
        ),
        "unknown_attachment" => (
            "附件不在核心里：先用 blob.put 传上来。",
            "The attachment is not in the core; upload it with blob.put first.",
        ),
        // 施工 W-2（`web-module.md`「给人看的字」）。
        "path_unreadable" => ("读不了这个路径。", "This path cannot be read."),
        "path_forbidden" => (
            "这是 Miyu 自己的数据，不给看。",
            "This is Miyu's own data and is not shown.",
        ),
        // 施工 W-4（`mermaid.md`「给人看的字」）。
        "mermaid_too_long" => ("这张图的源码太长了。", "The diagram source is too long."),
        "mermaid_failed" => ("这张图画不出来。", "The diagram could not be drawn."),
        // 施工 W-5（`web-module.md`「给人看的字」）。
        "too_many_uploads" => (
            "同时传的文件太多了，等前面的传完。",
            "Too many uploads at once; wait for the others to finish.",
        ),
        "upload_unknown" => (
            "没有这个上传，可能等太久作废了，重新传一次。",
            "No such upload; it may have expired. Upload the file again.",
        ),
        "upload_offset" => (
            "上传接不上，从核心说的地方接着传。",
            "The upload is out of step; continue from where the core says.",
        ),
        "upload_incomplete" => ("文件还没传完。", "The file is not fully uploaded yet."),
        // 施工 W-6（`web-module.md`「给人看的字」）。
        "unknown_blob" => ("找不到这份内容。", "This content cannot be found."),
        "not_running" => (
            "没有正在进行的回合，打断不了。",
            "No turn is running, so there is nothing to interrupt.",
        ),
        "turn_running" => (
            "有回合在进行：先打断，或者等它做完。",
            "A turn is running; interrupt it or wait for it to finish.",
        ),
        "unknown_turn" => (
            "没有这一轮，或者它已经撤掉了。",
            "There is no such turn, or it has already been undone.",
        ),
        "nothing_to_unrevert" => (
            "没有能恢复的撤销：没撤过，或者撤了以后又开过一轮、压缩过。",
            "There is nothing to restore: nothing was undone, or a turn or compaction came since.",
        ),
        "restoring" => (
            "正在撤销、恢复，等它做完再来。",
            "An undo or restore is still in progress; try again when it is done.",
        ),
        "nothing_to_revert" => ("没有能撤销的回合。", "There is no turn to undo."),
        "unknown_job" => (
            "没有这个任务，或者它已经结束了。",
            "There is no such job, or it has already ended.",
        ),
        "not_a_command" => (
            "这是子代理，不是后台命令：去看它的会话。",
            "This is a subagent, not a background command; open its session instead.",
        ),
        "nothing_to_compact" => (
            "没有能压的：还没压过的内容都在原样留着的最近一段里。",
            "Not enough to compact: everything not yet compacted is in the recent part that stays as it is.",
        ),
        // 2026-09-30 项目主人定（施工 6-8 补）：头把它当一条提示通知显示。
        "nothing_to_clear" => ("上下文为空", "The context is empty."),
        // 2026-09-30 项目主人定（施工 4-7 再补）：两种情况一句话，头把它当一条提示通知显示。
        "not_redoable" => ("无法重做", "Cannot redo."),
        // 施工 D-1：`session.answer` 碰得到的四个（`unknown_decision` 协议上碰不到，`protocol.md`「出错」）。
        "not_asking" => (
            "它没在等回答：已经答过，或者已经了结了。",
            "It is not waiting for an answer: it was answered or settled already.",
        ),
        "no_rule" => (
            "这一次只能允许这一次，或者拒绝。",
            "This one can only be allowed once or denied.",
        ),
        "unexpected_reason" => ("只有拒绝能带理由。", "Only a denial can carry a reason."),
        "bad_answer" => (
            "回答和题目对不上：几道题几条，只能选题目里的选项。",
            "The answers do not fit the questions: one per question, picking only their options.",
        ),
        // 2026-10-01 主会话定（施工 3-8 四补）。
        "nothing_to_recap" => ("还没有可回顾的内容", "There is nothing to recap yet."),
        // 施工 8-2（`config.md`「协议拒绝时的话」）。
        "unknown_config_key" => ("没有这一项配置。", "There is no such setting."),
        // 施工 8-3（`config.md`「协议拒绝时的话」）。
        "config_invalid" => (
            "配置有几处不对，没有改。",
            "Some settings are not right. Nothing was changed.",
        ),
        "config_conflict" => (
            "这一项刚被别处改过，没有改：先看看现在的值。",
            "This was just changed elsewhere. Nothing was changed. Look at the current value first.",
        ),
        "config_file_broken" => (
            "配置文件现在读不进来，没法只改一项：先把它改好，比如用 miyu config edit。",
            "The config file cannot be read right now, so a single setting cannot be changed. Fix the file first, e.g. with miyu config edit.",
        ),
        "no_project_config" => (
            "这个目录找不到项目配置。",
            "There is no project config for this directory.",
        ),
        "unknown_secret" => ("没有这个密钥。", "There is no such secret."),
        "unknown_provider" => ("没有这个供应商。", "There is no such provider."),
        "unknown_model" => (
            "配置里没有这个模型或者池。",
            "There is no such model or pool in the configuration.",
        ),
        "recap_failed" => (
            "回顾没写成：请求模型出错了。",
            "The recap could not be written: the model request failed.",
        ),
        // 施工 8-20（`models.md`「给人看的字」）。
        "no_model" => ("没有可用的模型。", "No model is available."),
        "cooling" => (
            "模型都在冷却，稍后再试。",
            "All models are cooling down; try again later.",
        ),
        "model_failed" => ("请求模型出错了。", "The model request failed."),
        // 施工 W-8（`web-module.md`「给人看的字」）。
        "bad_code" => (
            "这个一次性码用不了了：过期了，或者已经用过。再运行一次 miyu web。",
            "This one-time code no longer works: it expired or was already used. Run miyu web again.",
        ),
        "bad_login" => (
            "登录过期了，或者被退出了，用用户名和密码再登录一次。",
            "The login expired or was signed out. Sign in with your username and password.",
        ),
        "bad_password" => ("用户名或者密码不对。", "Wrong username or password."),
        "login_throttled" => (
            "错的次数太多了，过一分钟再试。忘了密码的话，在本机运行 miyu web --reset。",
            "Too many failed attempts. Try again in a minute. Forgot the password? Run miyu web --reset on this machine.",
        ),
        "setup_first" => ("先设好用户名和密码。", "Set a username and password first."),
        "local_only" => (
            "只有本机的终端能要一次性码。",
            "Only a terminal on this machine can ask for a one-time code.",
        ),
        _ => ("被拒绝了。", "Refused."),
    }
}
