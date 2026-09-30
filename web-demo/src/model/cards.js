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
  const ext = extOf(target);
  const kind = ext ? KINDS.find((k) => res.cards[k].includes(ext)) : null;
  if (!kind) return null;
  const name = decodeSafe(target.split(/[?#]/)[0].split('/').pop() ?? target);
  return { kind, target, label: labelled?.[1]?.trim() || name };
}

/**
 * `![说明](地址)` 画成什么（蓝图「图片」第 1 条）：照扩展名是图片、视频、音频的各画各的；没有扩展名的照图（写的就是图）；
 * 有扩展名、又不是这几种的（网页、文档）是 `null`：不当图，照链接写。
 * @param {string} target
 * @returns {'video'|'audio'|'image'|null}
 */
export function embedKind(target) {
  const ext = extOf(target);
  if (!ext) return 'image';
  return KINDS.find((k) => res.cards[k].includes(ext)) ?? null;
}

const KINDS = /** @type {const} */ (['video', 'audio', 'image']);

/** 地址的扩展名（小写，不带 `?`、`#` 后面的）；最后一截没有 `.` 的是 `null`。 @param {string} target */
function extOf(target) {
  const last = target.split(/[?#]/)[0].split('/').pop() ?? '';
  return /\.([a-z0-9]+)$/i.exec(last)?.[1]?.toLowerCase() ?? null;
}

/** `%xx` 换回字；写坏了的照原样。 */
function decodeSafe(s) {
  try { return decodeURIComponent(s); } catch { return s; }
}
