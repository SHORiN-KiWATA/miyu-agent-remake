// @ts-check
//! 在收的回答（蓝图 `web.md`「她的回答：Markdown」第 7 条，照旧版 `app.js:8059-8090`）：每来一段字整段重画，
//! 画之前把半截的写法补齐，免得一个没收尾的 `` ` `` 把后面全染成代码、收尾一到整段又跳回来（旧版 09-10 逐帧取证的「字在跳」）。
//!
//! 只补三样：没收齐的围栏、最后一段里单数的 `` ` ``、单数的 `**`。单个的 `*`、`_` 和列表、公式打架，不碰。
//! 补的字只进这一次画，不进任何存档：收完了照原文重画。

import { parse, FENCE_OPEN, FENCE_CLOSE } from './parse.js';

/**
 * 补齐半截的写法。围栏照 `parse.js` 的认法（`~~~` 不算；```` ```js ```` 不算收尾）；围栏里的字不参与配对。
 * @param {string} raw 收到这里的原文
 * @returns {string}
 */
export function stabilize(raw) {
  const text = String(raw ?? '');
  if (!text) return text;
  const lines = text.split('\n');
  let open = false;
  let tail = 0;
  lines.forEach((line, i) => {
    if (open ? FENCE_CLOSE.test(line) : FENCE_OPEN.test(line)) {
      open = !open;
      // 围栏一收，最后一段从它后面算
      if (!open) tail = i + 1;
    } else if (!open && !line.trim()) tail = i + 1;
  });
  if (open) return `${text}\n\`\`\``;
  const last = lines.slice(tail).join('\n');
  let patched = text;
  if ((last.match(/`/g) ?? []).length % 2) patched += '`';
  if ((last.match(/\*\*/g) ?? []).length % 2) patched += '**';
  return patched;
}

/**
 * 在收的回答的块：补齐以后解析；最后一块是代码的算还在写（`closed: false`），哪怕围栏是补上的、或者刚收齐——
 * 后面也许还有字，它先不上色（`highlight.js` 停一会儿再上），以后的 mermaid 也先不画。
 * @param {string} source 收到这里的原文
 * @param {{line?: (line: string) => any}} [options] 同 `parse`
 * @returns {import('./parse.js').Block[]}
 */
export function liveBlocks(source, options) {
  const blocks = parse(stabilize(source), options);
  const last = blocks.at(-1);
  if (last?.type === 'code') last.closed = false;
  return blocks;
}
