// @ts-check
//! 压缩的进度条怎么走（蓝图 `web.md`「压缩的进度」第 3 条，照 TUI 定的）：条分几格，显示的格子按整格一顿一顿地追真实的字数，
//! 永远不超过真实够的格数；没压好之前最多到 `cap`。纯函数，计时在 `ui/compacting.js`。

/**
 * 真实的字数够亮几整格：已写 ÷ 估计，最多到 `cap`；没给估计的是 0。
 * @param {number} written @param {number|null|undefined} expected @param {number} cells @param {number} cap
 */
export function realCells(written, expected, cells, cap) {
  if (!expected) return 0;
  return Math.floor(Math.min(cap, written / expected) * cells);
}

/**
 * 追一步：多走 1–3 格（`random` 交回 0 到 1），不超过真实够的格数；已经够了不动。
 * @param {number} shown @param {number} real @param {() => number} [random]
 */
export function chase(shown, real, random = Math.random) {
  if (shown >= real) return shown;
  return Math.min(real, shown + 1 + Math.floor(random() * 3));
}

/** 百分比照真实的字数，最多到 `cap`。 @param {number} written @param {number} expected @param {number} cap */
export function percent(written, expected, cap) {
  return Math.round(Math.min(cap, written / expected) * 100);
}
