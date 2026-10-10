//! 终端自己弹的通知（蓝图 `tui.md`「系统通知」第 4 条）：kitty 的 OSC 99、iTerm2 这几个认的 OSC 9。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// kitty 的 OSC 99：`a=focus` 点一下跳回这个窗口，`o=unfocused` 窗口在前台时 kitty 不弹，标题、正文、程序名都
/// base64（`e=1`），`s=silent` 音由界面自己放（kitty 的 `s=` 只认声音主题里的名字）。标题、正文两段用同一个 `id`，
/// 同一个界面的新通知顶掉旧的；两段要一次写完，不能被别的输出劈开。照旧版 09-18 验过的写法。
pub fn kitty(id: &str, app: &str, title: &str, body: &str) -> String {
    let b = |text: &str| STANDARD.encode(text);
    format!(
        "\x1b]99;i={id}:e=1:d=0:a=focus:o=unfocused:u=1:f={}:s={}:p=title;{}\x1b\\\x1b]99;i={id}:e=1:d=1:p=body;{}\x1b\\",
        b(app),
        b("silent"),
        b(title),
        b(body)
    )
}

/// OSC 9：iTerm2、WezTerm、ghostty 认，只有一段字。标题和正文拼成一句；字里的控制字符去掉，免得提前收尾。
pub fn osc9(title: &str, body: &str) -> String {
    let text: String = format!("{title}: {body}")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    format!("\x1b]9;{text}\x07")
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    use super::{kitty, osc9};

    #[test]
    fn kitty_gets_two_base64_parts_that_jump_back_and_stay_quiet() {
        let seq = kitty("miyu-42", "Miyu", "Miyu", "回答好了 · 3.2s");
        let parts: Vec<&str> = seq.split("\x1b\\").filter(|p| !p.is_empty()).collect();
        assert_eq!(parts.len(), 2, "标题、正文两段：{seq:?}");
        let head = parts[0].strip_prefix("\x1b]99;").unwrap();
        let (meta, title) = head.split_once(';').unwrap();
        for key in [
            "i=miyu-42",
            "e=1",
            "d=0",
            "a=focus",
            "o=unfocused",
            "p=title",
        ] {
            assert!(meta.split(':').any(|k| k == key), "少了 {key}：{meta}");
        }
        let sound = meta.split(':').find_map(|k| k.strip_prefix("s=")).unwrap();
        assert_eq!(STANDARD.decode(sound).unwrap(), b"silent");
        assert_eq!(STANDARD.decode(title).unwrap(), "Miyu".as_bytes());
        let (meta, body) = parts[1]
            .strip_prefix("\x1b]99;")
            .unwrap()
            .split_once(';')
            .unwrap();
        assert!(meta.contains("d=1") && meta.contains("p=body"));
        assert_eq!(STANDARD.decode(body).unwrap(), "回答好了 · 3.2s".as_bytes());
    }

    #[test]
    fn osc9_is_one_line_without_controls() {
        assert_eq!(osc9("Miyu", "出错\x07"), "\x1b]9;Miyu: 出错\x07");
    }
}
