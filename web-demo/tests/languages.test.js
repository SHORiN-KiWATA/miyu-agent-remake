// @ts-check
//! 界面语言（蓝图 `web.md`「界面语言」）：用哪一种（设置项、浏览器的语言）、`/language` 轮着换、缺的字退回；每种语言的
//! 字齐不齐：界面的字、命令的说明、软件包的字和名字、运行状态行的词，格和 `{占位}` 一样，少了哪样当场红（第 6 条）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { pick, options, languageSpec, settingOf, fromConfig } from '../src/kernel/language.js';
import { local, merge } from '../src/lib/text.js';
import { res, settle } from '../src/util/res.js';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const json = (p) => JSON.parse(readFileSync(here(`../${p}`), 'utf8'));
const TABLE = json('resources/languages.json');
const CODES = TABLE.languages.map((l) => l.code);
const BASE = TABLE.fallback;

test('表：中文、英文、日文三种（英文 2026-10-10 加），缺字退回中文；每种有名字、浏览器语言的前缀、页面的语言标签', () => {
  assert.deepEqual(CODES, ['zh', 'en', 'ja']);
  assert.equal(BASE, 'zh');
  for (const l of TABLE.languages) {
    assert.ok(l.name && l.tag && l.locales.length, l.code);
  }
});

test('用哪一种：设置项写了哪种用哪种；auto 照浏览器的语言依次认前缀，一个都认不出的用退回的那一种', () => {
  assert.equal(pick('auto', ['ja-JP', 'en'], TABLE).code, 'ja');
  assert.equal(pick('auto', ['fr', 'zh-TW'], TABLE).code, 'zh', '认第一个认得出的');
  assert.equal(pick('auto', ['en-US', 'zh-TW'], TABLE).code, 'en', '英文认得出');
  assert.equal(pick('auto', ['fr', 'de'], TABLE).code, 'zh', '都认不出用退回的');
  assert.equal(pick('auto', [], TABLE).code, 'zh');
  assert.equal(pick('ja', ['zh-CN'], TABLE).code, 'ja', '写了的不看浏览器');
  const ja = pick('ja', [], TABLE);
  assert.equal(ja.tag, 'ja-JP');
  assert.equal(ja.fallback, 'zh');
  assert.equal(ja.name, '日本語');
});

test('/language 的浮层：先是跟着浏览器（写它现在认成的那一种），再是表里每一种写自己的名字；设置项现在写的那一行是当前', () => {
  const auto = options('auto', ['ja-JP'], TABLE);
  assert.deepEqual(auto.items, [{ value: 'auto', name: '日本語', auto: true }, { value: 'zh', name: '中文', auto: false }, { value: 'en', name: 'English', auto: false }, { value: 'ja', name: '日本語', auto: false }]);
  assert.equal(auto.current, 0);
  assert.equal(options('auto', ['fr'], TABLE).items[0].name, '中文', '浏览器的认不出：跟着浏览器认成退回的那一种');
  assert.equal(options('ja', [], TABLE).current, 3);
  assert.equal(options('zh', [], TABLE).current, 1);
});

test('设置项：auto 加表里的每一种，出厂 auto，改了重新载入；分层照配置，写错的用下面一层的', () => {
  const spec = languageSpec(TABLE);
  assert.equal(spec.type, 'choice');
  assert.deepEqual(spec.choices, ['auto', 'zh', 'en', 'ja']);
  assert.equal(spec.default, 'auto');
  assert.equal(spec.applies, 'reload');
  assert.equal(settingOf(TABLE, {}, {}), 'auto');
  assert.equal(settingOf(TABLE, { language: 'zh' }, {}), 'zh', '发行版的');
  assert.equal(settingOf(TABLE, { language: 'zh' }, { language: 'ja' }), 'ja', '个人的盖发行版的');
  assert.equal(settingOf(TABLE, { language: 'zh' }, { language: 'fr' }), 'zh', '写错的退回下面一层');
});

test('按语言写的一块：挑这一种，没有的退回；不是按语言写的原样交回', () => {
  const lang = { code: 'ja', fallback: 'zh' };
  assert.equal(local({ zh: '撤销', ja: '取り消し' }, lang), '取り消し');
  assert.equal(local({ zh: '撤销' }, lang), '撤销');
  assert.deepEqual(local(['a', 'b'], lang), ['a', 'b']);
  assert.equal(local('原样', lang), '原样');
  assert.deepEqual(local({ rows: 5 }, lang), { rows: 5 }, '键不是语言代码的不算');
});

test('缺的字退回：一层层合，上面有的用上面的，没有的用下面的；一组（数组）整个换', () => {
  const base = { a: '中', b: { c: '中c', d: '中d' }, e: ['1', '2'] };
  const over = { a: '日', b: { c: '日c' }, e: ['x'] };
  assert.deepEqual(merge(base, over), { a: '日', b: { c: '日c', d: '中d' }, e: ['x'] });
});

/** 一份字的格：每一句的路径 → 它的 `{占位}`（排好序）；一组的每一个各算一格。 */
function shape(table, path = '', out = new Map()) {
  if (typeof table === 'string') out.set(path, [...table.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(','));
  else if (Array.isArray(table)) table.forEach((v, i) => shape(v, `${path}[${i}]`, out));
  else if (table && typeof table === 'object') for (const [k, v] of Object.entries(table)) shape(v, path ? `${path}.${k}` : k, out);
  return out;
}

/** 两份字的格对得上：一样的路径、一样的占位。 */
function sameShape(a, b, what) {
  const sa = shape(a);
  const sb = shape(b);
  const missing = [...sa.keys()].filter((k) => !sb.has(k));
  const extra = [...sb.keys()].filter((k) => !sa.has(k));
  assert.deepEqual(missing, [], `${what} 少了`);
  assert.deepEqual(extra, [], `${what} 多了`);
  for (const [k, v] of sa) assert.equal(sb.get(k), v, `${what} 的 ${k} 占位对不上`);
}

test('界面的字：每种语言一份，格和占位和退回的那一份一模一样', () => {
  const base = json(`resources/text/${BASE}.json`);
  for (const code of CODES) sameShape(base, json(`resources/text/${code}.json`), `text/${code}.json`);
});

test('命令的说明：每条每种语言各写一句', () => {
  for (const c of json('resources/commands.json').commands) {
    for (const code of CODES) assert.ok(typeof c.summary?.[code] === 'string' && c.summary[code], `/${c.name} 没有 ${code} 的说明`);
  }
});

test('软件包：名字每种语言都有；字每种语言一份，格和占位对得上；不再写 zh-CN', () => {
  for (const id of readdirSync(here('../packages'))) {
    const m = json(`packages/${id}/manifest.json`);
    for (const code of CODES) assert.ok(m.name?.[code], `${id} 的名字没有 ${code}`);
    assert.ok(!JSON.stringify(m).includes('"zh-CN"'), `${id} 的清单还写着 zh-CN`);
    if (!m.text) continue;
    for (const code of CODES) sameShape(m.text[BASE], m.text[code] ?? {}, `${id} 的 text.${code}`);
  }
});

test('运行状态行的词：每一档每种语言各一组', () => {
  const tiers = json('packages/pulse/manifest.json').settings.words.default.tiers;
  for (const tier of tiers) {
    for (const code of CODES) assert.ok(Array.isArray(tier.words[code]) && tier.words[code].length, `第 ${tier.after} 秒那一档没有 ${code}`);
  }
});

test('时间线收起那一行：跟着浏览器（auto）的挑核心给的英文那一份，手动定了语言的挑那种语言的（2026-10-01 项目主人定）', async () => {
  assert.equal(TABLE.auto_summary, 'en');
  assert.equal(pick('auto', ['ja-JP'], TABLE).summary, 'en');
  assert.equal(pick('ja', [], TABLE).summary, 'ja');
  assert.equal(pick('zh', ['ja-JP'], TABLE).summary, 'zh');
  const { itemsOf } = await import('../src/model/entries.js');
  const zh = json('resources/text/zh.json');
  const ja = json('resources/text/ja.json');
  const lang = (code, summary) => ({ ...TABLE.languages.find((l) => l.code === code), fallback: 'zh', summary });
  const group = [{ id: 'g1', kind: 'group', turn: 1, steps: [], summary: [{ text: '执行了 1 条命令' }], summary_en: [{ text: 'Ran 1 command' }] }];
  const line = () => itemsOf(group, { state: 'idle' }).items[0].summary.spans[0].text;
  settle(zh, ja, lang('ja', 'en'), []);
  assert.equal(line(), 'Ran 1 command');
  assert.equal(res.text.timeline.thought, ja.timeline.thought, '别的字照界面语言');
  settle(zh, ja, lang('ja', 'ja'), []);
  assert.equal(line(), '执行了 1 条命令', '手动定的照核心照连接的语言写的那一份');
});

test('个人设置写了网页的表里没有的语言：界面照浏览器认，收起那一行照界面那一种；写了 en 的就是英文（2026-10-10 起表里有）（蓝图「界面语言」第 5 条）', () => {
  const table = JSON.parse(readFileSync(new URL('../resources/languages.json', import.meta.url), 'utf8'));
  const en = pick('en', ['ja-JP'], table);
  assert.equal(en.code, 'en');
  assert.equal(en.summary, 'en');
  const ko = pick('ko', ['ja-JP'], table);
  assert.equal(ko.code, 'ja');
  assert.equal(ko.summary, 'ja');
  assert.equal(pick('fr', ['zh-CN'], table).summary, 'zh', '连收起那一行的字都没有的：照界面那一种');
});

test('从核心读个人设置的界面语言：config.get 的 items、config.changed 的 keys（取 effective）；没有的是 null', () => {
  assert.equal(fromConfig({ items: { 'ui.language': { value: 'ja', origin: { layer: 'personal' } } } }), 'ja');
  assert.equal(fromConfig({ keys: { 'ui.language': { effective: 'zh', value: 'zh', applies: 'now' } } }), 'zh');
  assert.equal(fromConfig({ keys: { 'ui.language': { effective: 'auto', applies: 'now' } } }), 'auto', '删掉了的那一项照 effective（回到默认）');
  assert.equal(fromConfig({ keys: { 'log.level': { effective: 'info' } } }), null);
  assert.equal(fromConfig(null), null);
});
