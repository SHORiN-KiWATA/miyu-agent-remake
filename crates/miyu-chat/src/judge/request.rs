//! 拼判官的请求（`chat.md` 第六条「怎么走」第 1、2 条）：十三份原文照先后接起来，每份之间不加别的字。

use std::collections::BTreeMap;

use super::{Ask, JudgeTexts, Message, Mode, Role};

/// 拼成 `model.call` 的 `messages`：一条 `system`、一条 `user`。
///
/// - system：`system.txt`；有人格的，人格的说明夹在 `persona-open.txt`、`persona-close.txt` 中间；打分的接 `reply.txt`，
///   只查违规的接 `moderation-only.txt`；`violations.txt`（`{severity_min}` 换成门槛）；`answer.txt`。
/// - user：群聊记录、这一条各夹在自己的标签中间；有 base64 解出来的字的，最后再夹一段。
///
/// 夹进标签的字末尾没有换行的补一个，收尾的标签才落在自己那一行；空的不补，免得标签中间多一个空行。夹进来的字
/// 原样用，不再转义：群聊记录和这一条由渲染器转义过，人格的说明是可信的。
///
/// # Panics
///
/// 不会：`violations` 换不换得出，[`JudgeTexts::new`] 造的时候已经试过；这里的 `expect` 只是那一步的证明。
pub fn request(texts: &JudgeTexts, ask: &Ask) -> Vec<Message> {
    let mut system = texts.system.clone();
    if let Some(persona) = &ask.persona {
        wrap(
            &mut system,
            &texts.persona_open,
            persona,
            &texts.persona_close,
        );
    }
    system.push_str(match ask.mode {
        Mode::Reply => &texts.reply,
        Mode::ModerationOnly => &texts.moderation_only,
    });
    let min = ask.severity_min.to_string();
    let fields = BTreeMap::from([("severity_min", min.as_str())]);
    // `JudgeTexts::new` 拿这个字段试换过，这里换得出。
    system.push_str(&texts.violations.render(&fields).expect("造的时候试换过"));
    system.push_str(&texts.answer);

    let mut user = String::new();
    wrap(
        &mut user,
        &texts.records_open,
        &ask.records,
        &texts.records_close,
    );
    wrap(
        &mut user,
        &texts.current_open,
        &ask.current,
        &texts.current_close,
    );
    if let Some(decoded) = &ask.decoded {
        wrap(
            &mut user,
            &texts.decoded_open,
            decoded,
            &texts.decoded_close,
        );
    }

    vec![
        Message {
            role: Role::System,
            text: system,
        },
        Message {
            role: Role::User,
            text: user,
        },
    ]
}

/// 把 `body` 夹在开头、收尾两份中间接到 `out` 后面；`body` 不空、末尾没有换行的补一个。
fn wrap(out: &mut String, open: &str, body: &str, close: &str) {
    out.push_str(open);
    out.push_str(body);
    if !body.is_empty() && !body.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(close);
}
