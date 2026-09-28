use super::call;

#[test]
fn deepseek_flash_views_images_but_not_pdf() {
    // DeepSeek 从 2026-08-21 起收图（施工 4-13）；PDF 不收。
    let call = call();
    assert_eq!(call.model.as_str(), "deepseek-flash");
    assert!(call.inputs.images);
    assert!(!call.inputs.pdf);
    assert_eq!(call.max_output, None);
}
