//! 占位的几句：读得进来、字段换得进去、少了字段的报错。出厂的那几份在 `tests/texts.rs` 里读。

use super::*;

fn sources<'a>(file_omitted: &'a str) -> DriverTextSources<'a> {
    DriverTextSources {
        image_omitted: "no image\n",
        file_omitted,
        no_output: "nothing\n",
        tool_attachments: "attachments:\n",
        tool_attachments_only: "see below\n",
    }
}

#[test]
fn the_file_name_is_filled_in_and_escaped() {
    let texts = DriverTexts::new(sources("file {name} ({media_type})\n")).unwrap();
    assert_eq!(
        texts.file_omitted("报告.pdf", "application/pdf"),
        "file 报告.pdf (application/pdf)\n"
    );
    // 文件名是不可信的字，照模板的规矩转义，伪造不了标签。
    assert!(!texts.file_omitted("<x>", "application/pdf").contains('<'));
    assert_eq!(texts.image_omitted(), "no image\n");
    assert_eq!(texts.no_output(), "nothing\n");
    assert_eq!(texts.tool_attachments(), "attachments:\n");
    assert_eq!(texts.tool_attachments_only(), "see below\n");
}

#[test]
fn a_field_that_does_not_belong_is_refused() {
    assert!(DriverTexts::new(sources("file {path}\n")).is_err());
    assert!(DriverTexts::new(sources("file {name\n")).is_err());
}
