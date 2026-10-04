//! mermaid 图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）：SVG 由核心画（`mermaid.render`），界面换好主题色
//! （`diagrams.rs`）交到这里；栅格化和回答里写的 `<svg>` 走同一条路（`svg.rs`），这里另存一份垫了底的 SVG，给图下面那
//! 一行「点开看大图」。

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

/// 点开看的大图：`svg` 最前面垫一块整张图大的 `backdrop` 底（不然看图程序的白底上浅色的字看不见），写进 `dir`（同一份
/// 只写一次），交回文件的路径；目录里多过 `keep` 张时删最早写的。
///
/// # Errors
///
/// 不像 SVG（找不到 `<svg …>`）、写不进目录时返回原因。
pub fn zoom(svg: &str, backdrop: (u8, u8, u8), dir: &Path, keep: usize) -> Result<PathBuf, String> {
    let mut hasher = DefaultHasher::new();
    (svg, backdrop).hash(&mut hasher);
    let file = dir.join(format!("{:016x}.svg", hasher.finish()));
    if !file.is_file() {
        let floored = floor(svg, backdrop).ok_or("不像 SVG")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::write(&file, floored).map_err(|e| e.to_string())?;
        prune(dir, keep);
    }
    Ok(file)
}

/// 在 `<svg …>` 开头那个标签后面垫一块铺满的底。
fn floor(svg: &str, (r, g, b): (u8, u8, u8)) -> Option<String> {
    let open = svg.find("<svg")?;
    let end = open + svg[open..].find('>')? + 1;
    let rect = format!(
        r##"<rect x="0" y="0" width="100%" height="100%" fill="#{r:02x}{g:02x}{b:02x}"/>"##
    );
    Some(format!("{}{rect}{}", &svg[..end], &svg[end..]))
}

/// 只留最近写的 `keep` 张 SVG。删不掉的不管：下次再删。
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .filter_map(|p| Some((fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, old) in files.into_iter().skip(keep.max(1)) {
        if fs::remove_file(&old).is_err() {
            // 删不掉（权限、正被占着）：留着，下次写图时再试。
        }
    }
}

#[cfg(test)]
mod tests;
