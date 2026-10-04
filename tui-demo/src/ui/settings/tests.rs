//! TestBackend 检查页面图层、密钥掩码和配置页光标，不创建 MIYU_HOME。
use super::draw;
use crate::{
    config::Config,
    input::Editor,
    settings::{Editing, Settings, data, forms},
};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

fn page() -> Settings {
    let mut page = Settings::new(false, true);
    page.loaded = true;
    page.providers = data::providers(&json!({"providers":[{"id":"relay","name":"Relay","models":[
        {"model":"text","ref":"relay/text","facts":{"inputs":{"value":["text"]}}},
        {"model":"vision","ref":"relay/vision","facts":{"inputs":{"value":["text","image"]}}}
    ]}]}));
    page.uses = json!({"chat":"relay/text","vision":"relay/vision"});
    page
}
fn render(page: &mut Settings, width: u16, height: u16) -> (String, crate::caret::Caret) {
    let config = Config::builtin().unwrap();
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut caret = crate::caret::Caret::default();
    term.draw(|frame| caret = draw(frame, page, &config))
        .unwrap();
    let buffer = term.backend().buffer();
    let screen = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (screen, caret)
}
#[test]
fn provider_default_and_pool_pages_render_at_wide_and_narrow_sizes() {
    for width in [100, 40] {
        let mut p = page();
        let (screen, _) = render(&mut p, width, 25);
        assert!(screen.contains("Relay"));
        assert!(screen.contains("vision"));
        assert!(
            p.hits
                .iter()
                .filter(|(_, hit)| matches!(hit, crate::settings::Hit::Tab(_)))
                .count()
                == 4
        );
        p.tab = 1;
        let (screen, _) = render(&mut p, width, 25);
        assert!(screen.contains("relay/text"));
        assert!(screen.contains("●"));
        p.tab = 3;
        p.pools = data::pools(
            &json!({"pools":[{"name":"daily","strategy":"rotate","models":["relay/vision"]}]}),
        );
        let (screen, _) = render(&mut p, width, 25);
        assert!(screen.contains("daily"));
        assert!(screen.contains("rotate"));
    }
}
#[test]
fn secret_values_and_editor_text_never_reach_the_terminal_buffer() {
    let mut p = page();
    p.form = Some(forms::provider(&json!({}), Some(&p.providers[0])));
    p.form.as_mut().unwrap().fields[3].value = "SECRET-FORM".into();
    let (screen, _) = render(&mut p, 100, 25);
    assert!(!screen.contains("SECRET-FORM"));
    assert!(screen.contains("••••"));
    let mut editor = Editor::default();
    editor.set("SECRET-EDITOR");
    p.editing = Some(Editing {
        field: 3,
        editor,
        options: vec![],
        selected: 0,
        checked: None,
        secret: true,
    });
    let (screen, caret) = render(&mut p, 40, 25);
    assert!(!screen.contains("SECRET-EDITOR"));
    assert!(screen.contains("••••"));
    assert!(caret.shown);
    assert!(caret.at.x < 40 && caret.at.y < 20);
}
#[test]
fn option_focus_parks_in_the_configuration_page_and_handles_tiny_screens() {
    let mut p = page();
    p.form = Some(forms::provider(&json!({}), Some(&p.providers[0])));
    p.editing = Some(Editing {
        field: 2,
        editor: Editor::default(),
        options: vec!["openai-chat".into(), "anthropic".into()],
        selected: 1,
        checked: None,
        secret: false,
    });
    let (screen, caret) = render(&mut p, 100, 25);
    assert!(screen.contains("anthropic"));
    assert!(!caret.shown);
    assert!(caret.at.y < 10);
    assert!(
        p.hits
            .iter()
            .any(|(_, hit)| matches!(hit, crate::settings::Hit::Option(1)))
    );
    for (width, height) in [(1, 1), (2, 3), (40, 5)] {
        let (_, caret) = render(&mut p, width, height);
        assert!(caret.at.x < width && caret.at.y < height);
    }
}
