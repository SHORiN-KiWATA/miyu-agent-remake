// @ts-check
//! Ctrl+S 暂存（蓝图 `web.md`「按键」，照 `tui.md`「按键」）：框里有字，存起来、清空；空着，取回来；两边都有，互换；两边都空，
//! 什么都不做。存的是字和框里的文件块（`[名字] → 路径`），附件留在框里不动。纯函数，框在 `ui/composer.js`。

/** @typedef {{text: string, blocks: [string, string][]}} Draft 框里的字和文件块 */

/**
 * @param {Draft} input 框里现在的
 * @param {Draft|null} stashed 暂存着的
 * @returns {{input: Draft, stashed: Draft|null, said: 'stashed'|'restored'|'swapped'|'empty'}} 换完以后框里的、暂存的，提示哪一句
 */
export function stash(input, stashed) {
  const has = input.text.trim() !== '';
  if (has && stashed) return { input: stashed, stashed: input, said: 'swapped' };
  if (has) return { input: { text: '', blocks: [] }, stashed: input, said: 'stashed' };
  if (stashed) return { input: stashed, stashed: null, said: 'restored' };
  return { input, stashed: null, said: 'empty' };
}
