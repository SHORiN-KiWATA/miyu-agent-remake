// @ts-check
//! 问核心画 mermaid（蓝图 `web.md`「mermaid 图」，核心施工 W-4 的 `mermaid.render`）：回的 SVG 里字、线、连线标签的垫底是三种记号色
//! （回应的 `marks`），换成这个包设置项 `colors` 里的 CSS 变量，换主题不用重画。画不出的一律交回 `null`，照代码块写。
//! 不碰页面，单测得到。

/**
 * 把 SVG 文字里的记号色换成页面的颜色：属性、`style`、`<style>` 里的都换，大小写不分；别的颜色不动。
 * @param {string} svg
 * @param {Record<string, string>} marks 记号 → 记号色（`#rrggbb`）
 * @param {Record<string, string>} colors 记号 → 页面上的颜色（`var(--text)` 这类）
 */
export function paint(svg, marks, colors) {
  let out = svg;
  for (const [name, mark] of Object.entries(marks ?? {})) {
    const to = colors[name];
    if (typeof mark !== 'string' || !to) continue;
    const pattern = new RegExp(`${mark.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?![0-9a-f])`, 'gi');
    out = out.replace(pattern, to);
  }
  return out;
}

/**
 * 一个问核心画图的函数：同一份源码只问一次（这个包这一次加载里记着）。
 * @param {(method: string, params: any) => Promise<any>} request 发给核心
 * @param {() => Record<string, string>} colors 现在的设置项 `colors`
 * @param {(message: string) => void} log 画不出时记一笔原因
 * @returns {(source: string) => Promise<string|null>}
 */
export function drawer(request, colors, log) {
  /** @type {Map<string, Promise<string|null>>} */
  const drawn = new Map();
  return (source) => {
    let got = drawn.get(source);
    if (!got) {
      got = request('mermaid.render', { source }).then((r) => {
        if (typeof r?.svg !== 'string') throw new Error('回应里没有 svg');
        return paint(r.svg, r.marks, colors());
      }).catch((err) => {
        log(`mermaid 画不出来：${err.reason ?? ''} ${err.message}`.trim());
        return null;
      });
      drawn.set(source, got);
    }
    return got;
  };
}
