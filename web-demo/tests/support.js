// @ts-check
//! 测试用的底料：从磁盘读资源、给人看的字；造事件、时刻。
//!
//! 只在 node 里跑：`node --test web-demo/tests`。时刻按 UTC 读（收尾那一行写本地时间）。

import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { res, settle } from '../src/util/res.js';

process.env.TZ = 'UTC';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));

/** 把 `resources/` 里的字、布局、时间线的配置装进 `res`，和页面里读到的一样；给人看的字照桥给的样子拼。 */
export function loadRes() {
  const json = (p) => JSON.parse(readFileSync(here(`../resources/${p}`), 'utf8'));
  Object.assign(res, {
    layout: json('layout.json'),
    timeline: json('timeline.json'),
    markdown: json('markdown.json'),
    artifacts: json('artifacts.json'),
    cards: json('cards.json'),
    languages: json('languages.json'),
    media: json('media.json'),
    human: human(),
  });
  // 界面的字、命令的说明照中文装，收起那一行是英文（和出厂的 auto、浏览器是中文时一样）
  settle(json('text/zh.json'), null, { ...res.languages.languages[0], fallback: 'zh', summary: 'en' }, json('commands.json').commands, json('text/en.json'));
  return res;
}

/**
 * 仓库资源目录里给人看的字，拼成桥 `web.human` 回的样子（`bridge/src/human.rs`）：内核的一份、每个软件包各一份，
 * 说法的编号前面加上它在哪（`core/…`、`software/<包>/…`）。
 */
function human() {
  const root = here('../../resources/');
  const read = (dir) => JSON.parse(readFileSync(`${root}${dir}/human/zh.json`, 'utf8'));
  const out = { tools: {}, said: {} };
  const add = (dir) => {
    const file = read(dir);
    Object.assign(out.tools, file.tools ?? {});
    for (const [k, v] of Object.entries(file.said ?? {})) out.said[`${dir}/${k}`] = v;
  };
  add('core');
  // 没有工具的软件包（mermaid、net 这类）没有给人看的字，跳过（核心也是照有没有这一份读）
  for (const p of readdirSync(`${root}software`).sort()) if (existsSync(`${root}software/${p}/human/zh.json`)) add(`software/${p}`);
  return out;
}

/** 样本之外造的事件：`at` 是从 10:00:00 起的秒数。 */
export function ev(seq, at, kind, turn, body, by = { kind: 'kernel' }) {
  return { seq, at: new Date(Date.UTC(2026, 8, 29, 10, 0, 0) + at * 1000).toISOString(), kind, turn, by, body };
}

/** 从 10:00:00 起 `s` 秒的那一刻（毫秒）。 */
export const ms = (s) => Date.UTC(2026, 8, 29, 10, 0, 0) + s * 1000;
