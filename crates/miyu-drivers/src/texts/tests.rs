//! 占位的几句：读得进来、字段换得进去、少了字段的报错。出厂的那几份经策略快照读，在
//! `crates/miyu-policy/src/snapshot/tests.rs` 里查。

use super::*;

/// 文本文件的三句（施工 3-9 三补），测试自己写的。
const TEXT_FILE: TextFileSources<'static> = TextFileSources {
    file_open: "<f {name}>\n",
    file_cut: "cut {shown}/{total}\n",
    file_close: "</f>\n",
};

/// 带名字的图片的三句（施工 3-9 四补），测试自己写的。
const IMAGE_NAME: ImageNameSources<'static> = ImageNameSources {
    image_open: "<i {name}>\n",
    image_close: "</i>\n",
    image_omitted_named: "no image {name}\n",
};

fn sources<'a>(file_omitted: &'a str) -> DriverTextSources<'a> {
    DriverTextSources {
        image_omitted: "no image\n",
        file_omitted,
        no_output: "nothing\n",
        tool_attachments: "attachments:\n",
        tool_attachments_only: "see below\n",
        text_file: Some(TEXT_FILE),
        image_name: Some(IMAGE_NAME),
    }
}

#[test]
fn the_file_name_is_filled_in_and_escaped() {
    let texts = DriverTexts::new(sources("file {name} ({media_type}, {size})\n")).unwrap();
    assert_eq!(
        texts.file_omitted("报告.pdf", "application/pdf", 1234),
        "file 报告.pdf (application/pdf, 1234)\n"
    );
    // 文件名是不可信的字，照模板的规矩转义，伪造不了标签。
    assert!(
        !texts
            .file_omitted("<x>", "application/pdf", 1)
            .contains('<')
    );
    assert_eq!(texts.image_omitted(None), "no image\n");
    assert_eq!(texts.no_output(), "nothing\n");
    assert_eq!(texts.tool_attachments(), "attachments:\n");
    assert_eq!(texts.tool_attachments_only(), "see below\n");
}

#[test]
fn an_older_placeholder_without_the_size_still_works() {
    // 以前造的快照里 `file-omitted` 没有 `{size}`（施工 3-9 三补以前）：照样换得出来，大小不写。
    let texts = DriverTexts::new(sources("file {name} ({media_type})\n")).unwrap();
    assert_eq!(
        texts.file_omitted("a.zip", "application/zip", 9),
        "file a.zip (application/zip)\n"
    );
}

#[test]
fn a_text_file_is_wrapped_with_its_name() {
    let texts = DriverTexts::new(sources("file {name}\n")).unwrap();
    assert_eq!(
        texts.text_file("a.md", "# 标题\n正文").as_deref(),
        Some("<f a.md>\n# 标题\n正文\n</f>\n"),
        "末尾没换行的补一个"
    );
    assert_eq!(
        texts.text_file("b.txt", "一行\n").as_deref(),
        Some("<f b.txt>\n一行\n</f>\n"),
        "有换行的不再补"
    );
    assert_eq!(
        texts.text_file("empty", "").as_deref(),
        Some("<f empty>\n</f>\n"),
        "空的只有开头收尾"
    );
    // 文件名照规矩转义，内容原样。
    let wrapped = texts.text_file("\"x\"", "<tag>").unwrap();
    assert!(wrapped.starts_with("<f \\u0022x\\u0022>\n"), "{wrapped}");
    assert!(wrapped.contains("\n<tag>\n"), "{wrapped}");
}

#[test]
fn a_long_text_file_says_how_much_was_cut() {
    let texts = DriverTexts::new(sources("file {name}\n")).unwrap();
    let long = "a".repeat(crate::text_file::LIMIT + 10);
    let wrapped = texts.text_file("big.log", &long).unwrap();
    let expected = format!(
        "<f big.log>\ncut 65536/65546\n{}\n</f>\n",
        "a".repeat(crate::text_file::LIMIT)
    );
    assert_eq!(wrapped, expected);
}

#[test]
fn without_the_three_texts_a_text_file_is_not_wrapped() {
    let texts = DriverTexts::new(DriverTextSources {
        text_file: None,
        ..sources("file {name}\n")
    })
    .unwrap();
    assert_eq!(texts.text_file("a.md", "x"), None);
}

#[test]
fn a_named_image_gets_tags_and_a_placeholder_with_its_name() {
    let texts = DriverTexts::new(sources("file {name}\n")).unwrap();
    assert_eq!(
        texts.image_tags(Some("晚霞.png")),
        Some(("<i 晚霞.png>\n".to_string(), "</i>\n".to_string()))
    );
    assert_eq!(texts.image_omitted(Some("晚霞.png")), "no image 晚霞.png\n");
    // 不带名字的照旧：前后什么都不加，占位是不带名字的那一句。
    assert_eq!(texts.image_tags(None), None);
    assert_eq!(texts.image_omitted(None), "no image\n");
    // 名字是人给的文件名，照规矩转义，伪造不了标签。
    let (open, _) = texts.image_tags(Some("\"><x.png")).unwrap();
    assert_eq!(open, "<i \\u0022\\u003e\\u003cx.png>\n");
    assert_eq!(
        texts.image_omitted(Some("<x>.png")),
        "no image \\u003cx\\u003e.png\n"
    );
}

#[test]
fn without_the_image_texts_a_named_image_is_written_as_before() {
    // 以前造的快照里没有那三句：带名字的图片照不带名字的写。
    let texts = DriverTexts::new(DriverTextSources {
        image_name: None,
        ..sources("file {name}\n")
    })
    .unwrap();
    assert_eq!(texts.image_tags(Some("a.png")), None);
    assert_eq!(texts.image_omitted(Some("a.png")), "no image\n");
}

#[test]
fn a_field_that_does_not_belong_is_refused() {
    assert!(DriverTexts::new(sources("file {path}\n")).is_err());
    assert!(DriverTexts::new(sources("file {name\n")).is_err());
    for broken in [
        TextFileSources {
            file_open: "<f {path}>\n",
            ..TEXT_FILE
        },
        TextFileSources {
            file_cut: "cut {name}\n",
            ..TEXT_FILE
        },
        TextFileSources {
            file_close: "</f {name}>\n",
            ..TEXT_FILE
        },
    ] {
        let sources = DriverTextSources {
            text_file: Some(broken),
            ..sources("file {name}\n")
        };
        assert!(DriverTexts::new(sources).is_err(), "{broken:?}");
    }
    for broken in [
        ImageNameSources {
            image_open: "<i {path}>\n",
            ..IMAGE_NAME
        },
        ImageNameSources {
            image_close: "</i {name}>\n",
            ..IMAGE_NAME
        },
        ImageNameSources {
            image_omitted_named: "no image {size}\n",
            ..IMAGE_NAME
        },
    ] {
        let sources = DriverTextSources {
            image_name: Some(broken),
            ..sources("file {name}\n")
        };
        assert!(DriverTexts::new(sources).is_err(), "{broken:?}");
    }
}
