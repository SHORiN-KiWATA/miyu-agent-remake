//! 读图纸：从 `docs/designs/01-架构.md` 的「代码的分层」一节，读出门禁要用的数据。
//!
//! 图纸就是真相源（01-架构 D4），门禁不另存一份层序表。图纸的格式被改坏了，
//! 这里说清楚读不出来的是什么。

/// 图纸在仓库里的位置。
pub const PATH: &str = "docs/designs/01-架构.md";

const SECTION: &str = "代码的分层";
const LAYER_HEADER: [&str; 5] = ["层", "名字", "放什么", "纯逻辑", "crate"];
const WHITELIST_HEADER: [&str; 2] = ["crate", "理由"];
const MAX_LINES_ANCHOR: &str = "文件行数上限";
const TOOLS_ANCHOR: &str = "工具 crate";

#[derive(Debug, PartialEq)]
pub struct Layer {
    pub level: u32,
    pub name: String,
    pub pure: bool,
    pub crates: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub struct Drawing {
    pub layers: Vec<Layer>,
    /// 纯逻辑的层能用的外部 crate。
    pub whitelist: Vec<String>,
    pub max_lines: usize,
    /// 不属于任何一层的工具 crate。
    pub tools: Vec<String>,
}

impl Drawing {
    /// 这个 crate 登记在哪一层。
    pub fn layer_of(&self, name: &str) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|layer| layer.crates.iter().any(|c| c == name))
    }

    pub fn is_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t == name)
    }
}

pub fn parse(text: &str) -> Result<Drawing, String> {
    let section = section(text)?;
    let tables = tables(&section);

    let layer_table = find_table(&tables, &LAYER_HEADER)?;
    let mut layers = Vec::new();
    for row in &layer_table.rows {
        layers.push(layer(row)?);
    }
    for (i, layer) in layers.iter().enumerate() {
        let expected = i as u32 + 1;
        if layer.level != expected {
            return Err(format!(
                "层号要从 1 开始连续往下写，第 {} 行写的是 {}",
                i + 1,
                layer.level
            ));
        }
    }

    let whitelist_table = find_table(&tables, &WHITELIST_HEADER)?;
    let whitelist = whitelist_table
        .rows
        .iter()
        .flat_map(|row| backticked(&row[0]))
        .collect();

    let max_lines_line = find_line(&section, MAX_LINES_ANCHOR)?;
    let after = max_lines_line
        .split_once(MAX_LINES_ANCHOR)
        .map_or("", |(_, rest)| rest);
    let max_lines =
        first_number(after).ok_or_else(|| format!("「{MAX_LINES_ANCHOR}」这一句里没有写数字"))?;

    let tools = backticked(find_line(&section, TOOLS_ANCHOR)?);

    let drawing = Drawing {
        layers,
        whitelist,
        max_lines,
        tools,
    };
    check_unique(&drawing)?;
    Ok(drawing)
}

/// 标题里带「代码的分层」的那一节，到下一个同级或更高级的标题为止。
fn section(text: &str) -> Result<Vec<&str>, String> {
    let mut lines = text.lines();
    lines
        .by_ref()
        .find(|line| line.starts_with("### ") && line.contains(SECTION))
        .ok_or_else(|| format!("找不到标题里带「{SECTION}」的一节"))?;
    Ok(lines
        .take_while(|line| !line.starts_with("### ") && !line.starts_with("## "))
        .collect())
}

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// 一节里所有的表格。表格是连续的、以 `|` 开头的行；第二行是分隔行。
fn tables(section: &[&str]) -> Vec<Table> {
    let mut tables = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in section.iter().chain(std::iter::once(&"")) {
        if line.trim_start().starts_with('|') {
            current.push(line);
        } else if !current.is_empty() {
            if current.len() >= 2 {
                tables.push(Table {
                    header: cells(current[0]),
                    rows: current[2..].iter().map(|row| cells(row)).collect(),
                });
            }
            current.clear();
        }
    }
    tables
}

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

fn find_table<'a>(tables: &'a [Table], header: &[&str]) -> Result<&'a Table, String> {
    tables
        .iter()
        .find(|t| {
            t.header
                .iter()
                .map(String::as_str)
                .eq(header.iter().copied())
        })
        .ok_or_else(|| format!("找不到表头是「{}」的表格", header.join(" | ")))
}

fn layer(row: &[String]) -> Result<Layer, String> {
    let [level, name, _, pure, crates] = row else {
        return Err(format!(
            "层的表格每一行要有 5 格，这一行有 {} 格：{}",
            row.len(),
            row.join(" | ")
        ));
    };
    let level = level
        .parse()
        .map_err(|_| format!("「{level}」不是层号，层号写阿拉伯数字"))?;
    let pure = match pure.as_str() {
        "是" => true,
        "否" => false,
        other => {
            return Err(format!(
                "第 {level} 层「纯逻辑」一格写的是「{other}」，只能写「是」或「否」"
            ));
        }
    };
    Ok(Layer {
        level,
        name: name.clone(),
        pure,
        crates: backticked(crates),
    })
}

fn find_line<'a>(section: &[&'a str], anchor: &str) -> Result<&'a str, String> {
    section
        .iter()
        .copied()
        .find(|line| line.contains(anchor))
        .ok_or_else(|| format!("找不到带「{anchor}」的那一句"))
}

/// 一段文字里所有用反引号括起来的名字。
fn backticked(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn first_number(text: &str) -> Option<usize> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits: String = text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// 一个 crate 只能登记在一处。
fn check_unique(drawing: &Drawing) -> Result<(), String> {
    let mut seen: Vec<&str> = Vec::new();
    let names = drawing
        .layers
        .iter()
        .flat_map(|layer| &layer.crates)
        .chain(&drawing.tools);
    for name in names {
        if seen.contains(&name.as_str()) {
            return Err(format!("`{name}` 登记了不止一处，一个 crate 只属于一层"));
        }
        seen.push(name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
### 八、别的

| 层 | 名字 | 放什么 | 纯逻辑 | crate |
|---|---|---|---|---|
| 9 | 不相干 | 不在这一节 | 否 | `nope` |

### 九、代码的分层

| 层 | 名字 | 放什么 | 纯逻辑 | crate |
|---|---|---|---|---|
| 1 | 内核 | 事件 | 是 | `miyu-kernel` |
| 2 | 模块 | 组装 | 是 | `miyu-prompt`、`miyu-view` |
| 3 | 执行器 | 存储 | 否 | |

4. 文件行数上限：每个 `.rs` 文件不超过 500 行，测试也算在内。
5. 工具 crate 不属于任何一层。现在只有一个：`xtask`。

| crate | 理由 |
|---|---|
| `serde` | 序列化 |
| （还没有） | |

### 十、决定
";

    #[test]
    fn the_layering_section_is_read() {
        let drawing = parse(SAMPLE).unwrap();
        assert_eq!(drawing.layers.len(), 3);
        assert_eq!(
            drawing.layers[1],
            Layer {
                level: 2,
                name: "模块".into(),
                pure: true,
                crates: vec!["miyu-prompt".into(), "miyu-view".into()],
            }
        );
        assert!(drawing.layers[2].crates.is_empty());
        assert_eq!(drawing.whitelist, vec!["serde".to_string()]);
        assert_eq!(drawing.max_lines, 500);
        assert_eq!(drawing.tools, vec!["xtask".to_string()]);
        assert!(drawing.layer_of("nope").is_none(), "别的节里的表格不算");
    }

    #[test]
    fn the_real_drawing_parses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text = std::fs::read_to_string(root.join(PATH)).unwrap();
        let drawing = parse(&text).unwrap();
        assert_eq!(drawing.layers.len(), 6);
        assert_eq!(drawing.layer_of("miyu-kernel").map(|l| l.level), Some(1));
        assert!(drawing.is_tool("xtask"));
    }

    #[test]
    fn a_missing_section_is_reported() {
        let err = parse("### 九、别的\n").unwrap_err();
        assert!(err.contains("代码的分层"), "{err}");
    }

    #[test]
    fn a_bad_pure_cell_is_reported() {
        let text = SAMPLE.replace("| 1 | 内核 | 事件 | 是 |", "| 1 | 内核 | 事件 | 对 |");
        let err = parse(&text).unwrap_err();
        assert!(err.contains("「对」"), "{err}");
    }

    #[test]
    fn levels_must_count_up_from_one() {
        let text = SAMPLE.replace("| 2 | 模块 |", "| 5 | 模块 |");
        let err = parse(&text).unwrap_err();
        assert!(err.contains("写的是 5"), "{err}");
    }

    #[test]
    fn a_crate_registered_twice_is_reported() {
        let text = SAMPLE.replace("`miyu-view`", "`miyu-kernel`");
        let err = parse(&text).unwrap_err();
        assert!(err.contains("`miyu-kernel`"), "{err}");
    }

    #[test]
    fn a_missing_line_limit_is_reported() {
        let text = SAMPLE.replace("文件行数上限", "文件大小");
        let err = parse(&text).unwrap_err();
        assert!(err.contains("文件行数上限"), "{err}");
    }
}
