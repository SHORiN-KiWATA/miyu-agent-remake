// @ts-check
//! 测试用的底料：从磁盘读资源；拿仓库里事件的样本文件（`docs/designs/samples/events/`）拼成一个会话的日志。
//! 样本是核心写出来的真样子（`kernel/events-bodies.md`），测试照它断言，格式变了这里先红。
//!
//! 只在 node 里跑：`node --test web-demo/tests`。时刻按 UTC 读（收尾那一行写本地时间）。

import { readFileSync, readdirSync } from 'node:fs';
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
  for (const p of readdirSync(`${root}software`).sort()) add(`software/${p}`);
  return out;
}

/** 样本之外造的事件：`at` 是从 10:00:00 起的秒数。 */
export function ev(seq, at, kind, turn, body, by = { kind: 'kernel' }) {
  return { seq, at: new Date(Date.UTC(2026, 8, 29, 10, 0, 0) + at * 1000).toISOString(), kind, turn, by, body };
}

/** 从 10:00:00 起 `s` 秒的那一刻（毫秒）。 */
export const ms = (s) => Date.UTC(2026, 8, 29, 10, 0, 0) + s * 1000;

/**
 * 样本拼成的日志：所有种类的样本按序号排好，只要 `upto` 号以前的。1 到 84 号是一个连贯的会话：
 * 一轮看目录（42 号，后来撤销了）、一轮被打断（56 号，那时是只读）、一轮被拒绝以后照常说完（65 号）、
 * 一轮提问（76 号）；中间改过标题、权限。
 */
export function sampleLog(upto = 84) {
  const dir = here('../../docs/designs/samples/events/');
  const events = readdirSync(dir)
    .filter((f) => f.endsWith('.jsonl'))
    .flatMap((f) => readFileSync(dir + f, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)));
  return events.filter((e) => e.seq <= upto).sort((a, b) => a.seq - b.seq);
}
