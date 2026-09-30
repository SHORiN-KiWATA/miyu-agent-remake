// @ts-check
//! 对话区跟着最新的时停在哪（蓝图 `web.md`「对话区」的滚动那一行，规矩照 `tui.md`「正文」第 1 条）。纯函数。
//!
//! 跟着最新的时视口只往下走、不往回退：内容变短（时间线的一段收起、准备的那一行换掉）时停在到过的最深处，
//! 差的高度垫在内容底下，等她接下来的字填上。收得多、最新的内容要整个退到视口上面去的，视口跟着往上走，最新的底边最多退到
//! 视口从上往下 `keep` 那么深的地方（2026-10-01 项目主人指出：长的时间线收起到上面去了，视口没跟过去，只剩空白）。

/**
 * 视口停在哪、底下垫多高（px）。
 * @param {number} floor 跟着最新的时视口到过的最深处（`scrollTop`）
 * @param {number} view 视口多高
 * @param {number} natural 内容本来多高（不算垫的）
 * @param {number} [keep] 最新的内容的底边最多退到视口从上往下多深（px，`layout.json` 的 `follow_keep` 乘视口高）；不给的不管
 * @param {number} [base] 新的一屏从哪起（清空过的「上下文已清空」那一行）：跟着往上走也不越过它
 */
export function anchorAt(floor, view, natural, keep = -Infinity, base = 0) {
  const held = Math.min(floor, natural - keep);
  const top = Math.max(held, natural - view, base, 0);
  return { top, pad: padFor(top, view, natural) };
}

/**
 * 视口停在 `top`，内容底下要垫多高才滚得到那里。停在顶上的用不着垫：内容不到一屏高时不该多出一截能滚的空白。
 * @param {number} top
 * @param {number} view 视口多高
 * @param {number} natural 内容本来多高
 */
export const padFor = (top, view, natural) => (top > 0 ? Math.max(0, top + view - natural) : 0);

/**
 * 她的一段正文长过视口时停在哪（`tui.md`「正文」第 1 条后半）：跟着最新的要去的地方超过了正文的开头（留一点边），
 * 就停在开头上面一点，后面的字在下面接着长。
 * @param {number} target 跟着最新的要去哪（`scrollTop`）
 * @param {number|null} start 在写的那段正文从哪开始（在内容里的位置）；没有在写的是 `null`
 * @param {number} margin 开头上面留多少（`layout.json` 的 `pin_margin`）
 */
export function pinAt(target, start, margin) {
  if (start == null) return { top: target, pinned: false };
  const stop = Math.max(0, start - margin);
  return target > stop ? { top: stop, pinned: true } : { top: target, pinned: false };
}

/**
 * 离底边够不够近，算在底下（跟着最新的）。
 * @param {{scrollHeight: number, scrollTop: number, clientHeight: number}} box
 * @param {number} near 多近算在底下（`layout.json` 的 `follow_within`）
 */
export const atBottom = (box, near) => box.scrollHeight - box.scrollTop - box.clientHeight < near;
