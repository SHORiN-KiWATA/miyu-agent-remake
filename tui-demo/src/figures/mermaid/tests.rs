//! mermaid 出图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）：透明底、框不填色、按格子渲到位；
//! 点开看的大图带底、只写一次、多了删最早的。

use super::{Look, detail, draw, zoom};
use crate::figures::cells::Cell;
use crate::theme::DiagramColors;

const LOOK: Look = Look {
    colors: DiagramColors {
        text: (192, 202, 245),
        line: (122, 162, 247),
        backdrop: (26, 27, 38),
    },
    fonts: &[],
};

/// SVG 里的第一个 `<rect>` 是整张图的底。
fn backdrop(svg: &str) -> &str {
    let rect = &svg[svg.find("<rect").unwrap()..];
    &rect[..rect.find("/>").unwrap()]
}

#[test]
fn a_graph_becomes_svg_without_fills_and_bad_source_says_why() {
    let svg = detail("graph TD\nA[开始] -->|是| B[结束]", &LOOK, false).unwrap();
    assert!(svg.contains("<svg") && svg.contains("开始"));
    assert!(svg.contains("#c0caf5"), "字是主题的颜色");
    assert!(
        backdrop(&svg).contains(r#"fill="none""#),
        "正文里的图不铺底"
    );
    // 连线上的标签垫暗底，把线挡住。
    assert!(svg.contains(r##"fill="#1a1b26""##));
    let zoom = detail("graph TD\nA --> B", &LOOK, true).unwrap();
    assert!(
        backdrop(&zoom).contains(r##"fill="#1a1b26""##),
        "点开的大图带底"
    );
    assert!(detail("这不是图", &LOOK, false).is_err());
}

#[test]
fn it_renders_to_exactly_the_cells_it_takes_on_a_clear_background() {
    let cell = Cell {
        width: 10,
        height: 20,
    };
    let (image, fit) = draw("graph LR\nA --> B --> C", &LOOK, cell, 30, 200).unwrap();
    assert!(fit.cols <= 30);
    assert_eq!(
        (image.width(), image.height()),
        (u32::from(fit.cols) * 10, u32::from(fit.rows) * 20)
    );
    // 角上透明：终端自己的底透出来。
    assert_eq!(image.get_pixel(0, 0).0[3], 0);
    // 框里面也不填色：节点正中间透明（字画在中间一行，挑它上面一点）。
    let opaque = image.pixels().filter(|p| p.0[3] > 0).count();
    let total = (image.width() * image.height()) as usize;
    assert!(opaque * 4 < total, "大半是透明的：{opaque}/{total}");
}

#[test]
fn the_zoomed_copy_is_written_once_and_old_ones_are_dropped() {
    let dir = std::env::temp_dir().join(format!("miyu-tui-zoom-test-{}", std::process::id()));
    let first = zoom("graph TD\nA --> B", &LOOK, &dir, 2).unwrap();
    assert!(std::fs::read_to_string(&first).unwrap().contains("#1a1b26"));
    assert_eq!(zoom("graph TD\nA --> B", &LOOK, &dir, 2).unwrap(), first);
    for n in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        zoom(&format!("graph TD\nA --> N{n}"), &LOOK, &dir, 2).unwrap();
    }
    let left = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(left, 2, "只留最近的两张");
    assert!(!first.exists(), "最早的删了");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 看样子：把几种常见的图照终端的格子渲出来，铺在深色底上存成 PNG，给人看。
/// `MIYU_FIGURE_DUMP=<目录> cargo test --bin miyu-tui-demo dump_samples -- --ignored`
#[test]
#[ignore = "看样子用，要人看图"]
fn dump_samples() {
    let Some(dir) = std::env::var_os("MIYU_FIGURE_DUMP") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let samples = [
        (
            "flow",
            "graph TD\n  A[用户发来消息] --> B{需要调用工具吗?}\n  B -->|是| C[执行工具并读取结果]\n  B -->|否| D[直接回答]\n  C --> E[整理成回复]\n  D --> E",
        ),
        (
            "sequence",
            "sequenceDiagram\n  participant U as 用户\n  participant T as 终端界面\n  participant C as 核心\n  U->>T: 输入一句话\n  T->>C: session.send\n  C-->>T: model.delta\n  T-->>U: 画出回答\n  Note over T,C: 一行一条 JSON-RPC",
        ),
        (
            "class",
            "classDiagram\n  class Transcript {\n    +entries: Vec~Entry~\n    +start(turn)\n    +end()\n  }\n  class Entry {\n    +kind: Kind\n    +text: String\n  }\n  Transcript --> Entry",
        ),
        (
            "state",
            "stateDiagram-v2\n  [*] --> 空闲\n  空闲 --> 回答中: 发出一句话\n  回答中 --> 空闲: 一轮结束\n  回答中 --> 已打断: Esc 两下\n  已打断 --> 空闲",
        ),
    ];
    let cell = Cell {
        width: 11,
        height: 24,
    };
    // 字体照 `figures.json`，和程序里一样。
    let config = crate::config::Config::builtin().unwrap();
    let look = Look {
        colors: crate::theme::diagram_rgb(),
        fonts: &config.figures.fonts,
    };
    for (name, source) in samples {
        let (image, fit) = draw(source, &look, cell, 90, 200).unwrap();
        let mut canvas = image::RgbaImage::from_pixel(
            image.width(),
            image.height(),
            image::Rgba([26, 38, 76, 255]),
        );
        image::imageops::overlay(&mut canvas, &image, 0, 0);
        canvas.save(dir.join(format!("{name}.png"))).unwrap();
        std::fs::write(
            dir.join(format!("{name}.svg")),
            detail(source, &look, false).unwrap(),
        )
        .unwrap();
        println!("{name}: {}×{} 格", fit.cols, fit.rows);
    }
}
