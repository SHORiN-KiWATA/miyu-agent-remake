// @ts-check
//! 她的回答的 Markdown：入口（蓝图 `web.md`「她的回答：Markdown」，照旧版 `app.js:4949-5083`、`8092-8099`）。
//! 原文 → 块（`parse.js`，在收的先补齐：`stream.js`）→ DOM（`build.js`），整个换进容器。
//!
//! 每来一段字整段重画：页面一帧最多画一次（`ui/app.js` 的 `schedule`），回答的节点字变了就换（`ui/chat.js`）。
//! 以后加的东西从扩展点接进来（[`Hooks`]），这里和解析不用改。

import { parse } from './parse.js';
import { liveBlocks } from './stream.js';
import { build } from './build.js';

/**
 * 扩展点：以后加的东西接在这里，都可以不给。
 * @typedef {object} Hooks
 * @property {(lang: string, text: string, closed: boolean) => (Node|null)} [code] 围起来的代码：给一个节点就用它
 *   顶替平常的代码块，给 `null` 照平常的画（以后的 mermaid 图）。`closed` 是这块定了没有：围栏没收齐的、在收的回答里
 *   最后那一块都是 `false`，这时画出来的东西下一次重画就扔掉，贵的活（画图、取数）留到 `true` 再做
 * @property {(line: string) => (Node|null)} [line] 独占一行的字（原样，带着两头的空白）：给一个节点，这一行就单独画成
 *   它，段落在这里断开；给 `null` 照平常的写法认（以后的音视频、图片卡片）。每画一次每一行最多问一次，围栏里的不问
 * @property {(src: string, alt: string) => (Node|null)} [image] `![说明](地址)`：给一张图的节点（蓝图「图片」）；给 `null`
 *   照原文写
 * @property {(container: HTMLElement) => void} [after] 每画完一次调一次，拿到画好的容器（以后的链接卡片：找出
 *   独占一段的链接换成卡片）。这时容器可能还没放进页面（`ui/chat.js` 画好了才换上去）；在收的回答每来一段都会调，
 *   贵的活自己防抖，到点时先看容器还在不在页面上
 */

/**
 * 把一条回答画进容器（换掉里面原来的）。
 * @param {HTMLElement} container 回答的节点（`.markdown-body`）
 * @param {string} source 原文
 * @param {{streaming?: boolean, hooks?: Hooks, say?: (text: string, good?: boolean) => void}} [options]
 *   `streaming` 是还在收：先补齐半截的写法，最后一块代码算没定（第 7 条）；`say` 是提示（复制了几个字、复制不了）
 */
export function renderMarkdown(container, source, { streaming = false, hooks = {}, say } = {}) {
  const options = { line: hooks.line };
  const blocks = streaming ? liveBlocks(source, options) : parse(source, options);
  container.replaceChildren(build(blocks, { hooks, say }));
  hooks.after?.(container);
}
