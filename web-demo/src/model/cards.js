// @ts-check
//! 回答里单独一行的媒体地址（蓝图 `web.md`「音视频、图片卡片」，照旧版 `app.js:4846-4865`）：一行只写一个地址，或者
//! `[说明](地址)`，地址照扩展名（`resources/cards.json`）是视频、音频、图片的，这一行画成卡片。句子里的地址照链接写。

import { res } from '../util/res.js';

/** `[说明](地址)` 整行。 */
const LABELLED = /^\[([^\]\n]*)\]\(\s*([^()\s]+)\s*\)$/;
/** 光一个地址：网上的、本机的（绝对、`~/`、`file://`、相对），中间没有空白。 */
const BARE = /^(?:https?:\/\/|file:\/\/|~\/|\/|\.{0,2}\/?)[^\s<>()]+$/i;

/**
 * 这一行是不是一张卡片：是的交回种类、地址、说明（没写说明的用文件名），不是的是 `null`。
 * @param {string} line
 * @returns {{kind: 'video'|'audio'|'image', target: string, label: string}|null}
 */
export function mediaLine(line) {
  const text = (line ?? '').trim();
  const labelled = LABELLED.exec(text);
  const target = labelled ? labelled[2] : BARE.test(text) ? text : null;
  if (!target) return null;
  const ext = /\.([a-z0-9]+)(?:[?#].*)?$/i.exec(target)?.[1]?.toLowerCase();
  const kind = /** @type {const} */ (['video', 'audio', 'image']).find((k) => ext && res.cards[k].includes(ext));
  if (!kind) return null;
  const name = decodeSafe(target.split(/[?#]/)[0].split('/').pop() ?? target);
  return { kind, target, label: labelled?.[1]?.trim() || name };
}

/** `%xx` 换回字；写坏了的照原样。 */
function decodeSafe(s) {
  try { return decodeURIComponent(s); } catch { return s; }
}
