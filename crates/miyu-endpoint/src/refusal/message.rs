//! 拒绝时给人看的话（`docs/designs/04-核心协议.md` 第六节第 4 条）：一个原因码一句中文、一句英文。施工 W-8 从
//! `refusal.rs` 挪出来（那一份过了 500 行）。

/// 原因码 `reason` 的中文、英文。不认识的说「已拒绝」。
pub(super) fn of(reason: &str) -> (&'static str, &'static str) {
    match reason {
        "parse_error" => ("消息格式错误。", "Malformed message."),
        "invalid_request" => ("无效的请求。", "Invalid request."),
        "unknown_method" => ("方法不存在。", "Method not found."),
        "bad_params" => ("参数错误。", "Invalid parameters."),
        "internal_error" => (
            "内部错误，详见运行日志。",
            "Internal error. See the runtime log.",
        ),
        "hello_first" => ("请先握手（hello）。", "Send hello first."),
        "protocol_mismatch" => (
            "协议版本不兼容，请升级到同一版本。",
            "Protocol version mismatch. Upgrade to the same release.",
        ),
        "bad_token" => ("本机令牌无效。", "Invalid local token."),
        "unknown_persona" => ("人格不存在。", "Persona not found."),
        "persona_invalid" => ("人格文件有误。", "Invalid persona files."),
        "unknown_preset" => ("预设不存在。", "Preset not found."),
        "preset_invalid" => ("预设文件有误。", "Invalid preset file."),
        "not_a_directory" => ("不是目录。", "Not a directory."),
        "unknown_package" => ("软件包不存在。", "Package not found."),
        "package_exists" => (
            "编号与内置软件包重复。",
            "Id conflicts with a built-in package.",
        ),
        "package_required" => (
            "必需的软件包，无法卸载。",
            "Required package, cannot be removed.",
        ),
        "package_invalid" => ("清单有误，无法安装。", "Invalid manifest, cannot install."),
        "program_missing" => ("程序未安装。", "Program not installed."),
        "no_page" => ("没有后台页。", "No admin page."),
        "not_found" => ("文件不存在。", "File not found."),
        "program_not_running" => ("程序未运行。", "Program not running."),
        "method_timeout" => ("方法超时。", "Method timed out."),
        "unregistered" => ("方法未登记。", "Method not registered."),
        "method_failed" => ("方法执行失败。", "Method failed."),
        "avatar_not_image" => (
            "头像只支持 PNG、JPEG、WebP。",
            "Avatars must be PNG, JPEG or WebP.",
        ),
        "avatar_too_big" => (
            "头像超过 1 MiB 或 1024 像素。",
            "Avatar over 1 MiB or 1024 pixels.",
        ),
        "background_not_image" => (
            "背景图只支持 PNG、JPEG、WebP。",
            "Backgrounds must be PNG, JPEG or WebP.",
        ),
        "background_too_big" => (
            "背景图超过 5 MiB 或 4096 像素。",
            "Background over 5 MiB or 4096 pixels.",
        ),
        "not_switchable" => ("不能启用或停用。", "Cannot be enabled or disabled."),
        "not_an_extension" => ("不是扩展。", "Not an extension."),
        "extension_off" => ("扩展已停用。", "Extension disabled."),
        "needs_approval" => ("扩展权限未批准。", "Extension permissions not approved."),
        "session_not_found" => ("会话不存在。", "Session not found."),
        "unknown_call" => ("调用不存在。", "Call not found."),
        "no_system_account" => ("需要系统账号。", "System account required."),
        "venue_session" => (
            "平台会话不能直接发消息。",
            "Cannot send messages directly to a platform session.",
        ),
        "unknown_command" => ("命令不存在。", "Command not found."),
        "unknown_file" => ("不支持检查这个文件。", "This file cannot be checked."),
        "command_not_allowed" => (
            "仅终端管理员和群管理员可用。",
            "Terminal and group admins only.",
        ),
        "nothing_to_delete" => ("没有可删除的内容。", "Nothing to delete."),
        "persona_conflict" => (
            "人格已被修改，请刷新后重试。",
            "Persona was modified. Refresh and try again.",
        ),
        "preset_conflict" => (
            "预设已被修改，请刷新后重试。",
            "Preset was modified. Refresh and try again.",
        ),
        "owner_only" => ("仅终端管理员可用。", "Terminal admin only."),
        "session_stopped" => (
            "会话已停止，重新发送即可载入。",
            "Session stopped. Send again to reload.",
        ),
        "session_broken" => (
            "会话已损坏，无法载入。",
            "Session is corrupted and cannot be loaded.",
        ),
        "empty_message" => ("消息为空。", "Empty message."),
        "dir_too_wide" => ("目录范围过大。", "Directory too broad."),
        "attachment_unreadable" => ("无法读取文件。", "Cannot read file."),
        "attachment_too_big" => (
            "附件过大（最大 20 MiB，图片 5 MiB、8000 像素）。",
            "Attachment too large (max 20 MiB; images 5 MiB, 8000 px).",
        ),
        "attachment_in_data_root" => (
            "不能附加数据目录里的文件。",
            "Files in the data directory cannot be attached.",
        ),
        "unknown_attachment" => ("附件不存在。", "Attachment not found."),
        // 施工 W-2（`web-module.md`「给人看的字」）。
        "path_unreadable" => ("无法读取路径。", "Cannot read path."),
        "path_forbidden" => ("无权访问。", "Access denied."),
        // 施工 W-4（`mermaid.md`「给人看的字」）。
        "mermaid_too_long" => ("图表源码过长。", "Diagram source too long."),
        "mermaid_failed" => ("图表渲染失败。", "Diagram rendering failed."),
        // 施工 W-5（`web-module.md`「给人看的字」）。
        "too_many_uploads" => ("同时上传的文件过多。", "Too many uploads at once."),
        "upload_unknown" => ("上传已失效，请重新上传。", "Upload expired. Upload again."),
        "upload_offset" => ("上传位置不一致。", "Upload offset mismatch."),
        "upload_incomplete" => ("上传未完成。", "Upload incomplete."),
        // 施工 W-6（`web-module.md`「给人看的字」）。
        "unknown_blob" => ("内容不存在。", "Content not found."),
        "not_running" => ("没有进行中的回合。", "No turn in progress."),
        "turn_running" => ("回合进行中。", "A turn is in progress."),
        "unknown_turn" => ("回合不存在。", "Turn not found."),
        "nothing_to_unrevert" => ("没有可恢复的撤销。", "Nothing to restore."),
        "restoring" => ("正在撤销或恢复。", "Undo or restore in progress."),
        "nothing_to_revert" => ("没有可撤销的回合。", "Nothing to undo."),
        "memory_unavailable" => ("记忆不可用。", "Memory unavailable."),
        "memory_not_installed" => ("人格记忆未安装。", "Persona memory not installed."),
        "memory_busy" => ("正在整理记忆。", "Memory is being organized."),
        "dream_failed" => ("整理记忆失败。", "Memory organization failed."),
        "unknown_memory" => ("记忆不存在。", "Memory not found."),
        "memory_not_current" => ("记忆已失效。", "Memory no longer current."),
        "memory_too_long" => ("记忆过长。", "Memory too long."),
        "unknown_job" => ("任务不存在或已结束。", "Job not found or finished."),
        "not_a_command" => ("不是后台命令。", "Not a background command."),
        "nothing_to_compact" => ("没有可压缩的内容。", "Nothing to compact."),
        // 2026-09-30 项目主人定（施工 6-8 补）：头把它当一条提示通知显示。
        "nothing_to_clear" => ("上下文为空。", "Context is empty."),
        // 2026-09-30 项目主人定（施工 4-7 再补）：两种情况一句话，头把它当一条提示通知显示。
        "not_redoable" => ("无法重做。", "Cannot redo."),
        // 施工 O-14 上：`session.respond` 的两个。
        "not_ambient" => ("不是旁听消息。", "Not an overheard message."),
        // 施工 O-2 上：`provide` 的两个。
        "not_a_provider" => ("仅扩展可提供工具。", "Only extensions can provide tools."),
        "bad_tool" => ("工具规格有误。", "Invalid tool spec."),
        "already_answered" => ("已回复。", "Already answered."),
        // 施工 D-1：`session.answer` 碰得到的四个（`unknown_decision` 协议上碰不到，`protocol.md`「出错」）。
        "not_asking" => ("不在等待回答。", "Not waiting for an answer."),
        "no_rule" => ("只能允许本次或拒绝。", "Only allow once or deny."),
        "unexpected_reason" => ("仅拒绝可附理由。", "Only a denial can carry a reason."),
        "bad_answer" => ("回答与题目不匹配。", "Answers do not match the questions."),
        // 2026-10-01 主会话定（施工 3-8 四补）。
        "nothing_to_recap" => ("没有可回顾的内容。", "Nothing to recap."),
        // 施工 8-2（`config.md`「协议拒绝时的话」）。
        "unknown_config_key" => ("配置项不存在。", "Config key not found."),
        // 施工 8-3（`config.md`「协议拒绝时的话」）。
        "config_invalid" => ("配置有误，未保存。", "Invalid config, not saved."),
        "config_conflict" => (
            "配置已被修改，请刷新后重试。",
            "Config was modified. Refresh and try again.",
        ),
        "config_file_broken" => (
            "配置文件有误，请先修复。",
            "Config file is invalid. Fix it first.",
        ),
        "no_project_config" => ("未找到项目配置。", "Project config not found."),
        "unknown_secret" => ("密钥不存在。", "Secret not found."),
        "unknown_provider" => ("供应商不存在。", "Provider not found."),
        "unknown_model" => ("模型或模型池不存在。", "Model or pool not found."),
        "recap_failed" => ("生成回顾失败。", "Recap failed."),
        // 施工 8-20（`models.md`「给人看的字」）。
        "no_model" => ("没有可用的模型。", "No model is available."),
        "cooling" => (
            "模型冷却中，请稍后重试。",
            "Models cooling down. Try again later.",
        ),
        "model_failed" => ("模型请求失败。", "Model request failed."),
        // 施工 W-8（`web-module.md`「给人看的字」）。
        "bad_code" => ("一次性码已失效。", "One-time code expired."),
        "bad_login" => ("登录已失效，请重新登录。", "Login expired. Sign in again."),
        "bad_password" => ("用户名或密码错误。", "Incorrect username or password."),
        "login_throttled" => (
            "尝试次数过多，请一分钟后重试。",
            "Too many attempts. Try again in a minute.",
        ),
        "setup_first" => (
            "请先设置用户名和密码。",
            "Set a username and password first.",
        ),
        "local_only" => ("仅限本机。", "Local only."),
        _ => ("已拒绝。", "Refused."),
    }
}
