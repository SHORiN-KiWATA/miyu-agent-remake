// @ts-check
//! `@` 选文件（蓝图 `web.md`「`@` 选文件」，照 `tui.md`「`@` 文件列表」）：光标前面的 `@` 词、两种找法、选定以后写进话里的
//! 路径、`Tab` 进目录以后词怎么换、问不到时写什么。纯函数；列、找由桥做（`web.files`）。

import { t } from '../util/res.js';

/** JSON-RPC 的「没有这个方法」：桥太旧，没有 `web.files`，转给了核心 */
const NO_METHOD = -32601;

/**
 * 光标前面的 `@` 词：`@` 在最前面或者前面是空白（`a@b.com` 不算），到光标为止没有空白（`\ ` 算词里的空格）。交回词从哪起
 * （`@` 的位置）、到哪（光标）、`@` 后面的字（`\ ` 换回空格）；不是的交 `null`。
 * @param {string} value
 * @param {number} caret
 */
export function wordAt(value, caret) {
  const before = value.slice(0, caret);
  const m = /(?:^|\s)@((?:\\ |[^\s])*)$/.exec(before);
  if (!m) return null;
  const start = before.length - m[1].length - 1;
  return { start, end: caret, word: m[1].replace(/\\ /g, ' ') };
}

/**
 * 两种找法：带 `/` 的、`~`、`/` 打头的，按目录找（只读那一层：最后一个 `/` 前面是目录，后面是名字的开头）；空的列工作目录
 * 这一层；别的在工作目录里模糊找。
 * @param {string} word
 * @returns {{mode: 'dir', dir: string, prefix: string}|{mode: 'find', query: string}}
 */
export function plan(word) {
  if (word === '~') return { mode: 'dir', dir: '~', prefix: '' };
  if (word === '' || word.includes('/') || word.startsWith('~')) {
    const cut = word.lastIndexOf('/') + 1;
    return { mode: 'dir', dir: word.slice(0, cut), prefix: word.slice(cut) };
  }
  return { mode: 'find', query: word };
}

/**
 * 选定的文件、目录写进话里的样子：在工作目录里的写相对路径，家目录里的写 `~/…`，别处写绝对路径；目录后面带 `/`；带空白、
 * 引号的加单引号（照 shell，单引号写成 `'\''`）。
 * @param {string} full 绝对路径
 * @param {boolean} dir
 * @param {string|null} cwd
 * @param {string|null} home
 */
export function pathText(full, dir, cwd, home) {
  const under = (base) => base && (full === base || full.startsWith(base.endsWith('/') ? base : `${base}/`));
  let text = cwd && under(cwd) ? full.slice(cwd.length).replace(/^\//, '') : home && under(home) ? `~${full.slice(home.length)}` : full;
  if (dir && !text.endsWith('/')) text += '/';
  return /[\s'"]/.test(text) ? `'${text.replace(/'/g, "'\\''")}'` : text;
}

/** `Tab` 进目录：词换成这个目录（列表上写的那样，空格写成 `\ `），接着按目录找。 */
export function dirWord(path) {
  return `@${path.replace(/ /g, '\\ ')}`;
}

/**
 * 框里 `[start, end)` 换成 `text`：交回新的字和光标（放在换进去的后面）。
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {string} text
 */
export function splice(value, start, end, text) {
  return { value: value.slice(0, start) + text + value.slice(end), caret: start + text.length };
}

/**
 * 问不到的写什么（蓝图「`@` 选文件」第 6 条）：桥太旧（核心回「没有这个方法」）说重新编、重启桥；别的照原话。
 * @param {any} err
 */
export function failure(err) {
  return err?.code === NO_METHOD ? t('mention.no_api') : err?.message ?? String(err);
}
