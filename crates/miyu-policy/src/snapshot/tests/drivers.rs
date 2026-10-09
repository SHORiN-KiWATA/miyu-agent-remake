//! 驱动的占位进快照（施工 3-9 三补、四补、五补，`docs/blueprint/policy.md`「`CoreTexts`」的 `drivers`）：出厂的快照每一句各是
//! 各的；以前造的快照没有带名字的图片那三句，读回来一字不差，照不带名字的写。施工 3-9 五补从 `tests.rs` 挪出来：那一份到了
//! 行数上限。

use super::*;

#[test]
fn each_driver_placeholder_is_its_own() {
    let texts = engineer().driver_texts().unwrap();
    let drivers = core().drivers;
    assert_eq!(texts.image_omitted(None, None), drivers.image_omitted);
    assert_eq!(texts.no_output(), drivers.no_output);
    assert_eq!(texts.tool_attachments(), drivers.tool_attachments);
    assert_eq!(texts.tool_attachments_only(), drivers.tool_attachments_only);
    let omitted = texts.file_omitted("a.pdf", "application/pdf", 1234, None);
    assert!(
        omitted.contains("a.pdf") && omitted.contains("1234"),
        "{omitted}"
    );
    // 文本文件的三句（施工 3-9 三补）：出厂的快照带着，开头、截过的、收尾各是各的。
    let text = drivers.text_file.expect("出厂的带着");
    let wrapped = texts
        .text_file("a.md", &"x".repeat(70_000))
        .expect("有三句");
    assert!(wrapped.starts_with(&text.file_open.replace("{name}", "a.md")));
    assert!(
        wrapped.contains(
            &text
                .file_cut
                .replace("{shown}", "65536")
                .replace("{total}", "70000")
        )
    );
    assert!(wrapped.ends_with(&text.file_close));
    // 带名字的图片的三句（施工 3-9 四补）：出厂的快照带着，开头、收尾、占位各是各的。
    let image = drivers.image_name.expect("出厂的带着");
    assert_eq!(
        texts.image_tags(Some("a.png")),
        Some((
            image.image_open.replace("{name}", "a.png"),
            image.image_close
        ))
    );
    assert_eq!(
        texts.image_omitted(Some("a.png"), None),
        image.image_omitted_named.replace("{name}", "a.png")
    );
    // 看不了的附件带路径的两句（施工 3-9 五补）：出厂的快照带着。
    let attached = drivers.attached_path.expect("出厂的带着");
    assert_eq!(
        texts.image_omitted(Some("a.png"), Some("/a.png")),
        attached
            .image_omitted_path
            .replace("{name}", "a.png")
            .replace("{path}", "/a.png")
    );
    assert!(
        texts
            .file_omitted("a.pdf", "application/pdf", 1, Some("/a.pdf"))
            .contains("/a.pdf")
    );
}

/// 带名字的图片的三句（施工 3-9 四补）：以前造的快照里没有，读回来一字不差，带名字的图片照不带名字的写。
#[test]
fn older_snapshots_lack_the_image_name_texts() {
    let mut old = engineer();
    old.core.drivers.image_name = None;
    let bytes = String::from_utf8(old.to_bytes()).unwrap();
    assert!(!bytes.contains("image_name"), "没有的不写：{bytes}");
    assert_eq!(Snapshot::from_bytes(bytes.as_bytes()), Ok(old.clone()));
    let texts = old.driver_texts().unwrap();
    assert_eq!(texts.image_tags(Some("a.png")), None);
    assert_eq!(
        texts.image_omitted(Some("a.png"), None),
        texts.image_omitted(None, None)
    );
}
