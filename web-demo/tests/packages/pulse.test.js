// @ts-check
//! 运行状态行（蓝图 `web.md`「运行状态行」，照 `tui.md`「运行状态行和排队的消息」第 1–3 条和 TUI 演示 `pulse/tests.rs`、
//! `ui/status.rs` 的测试）：一阵事件忙完再换词、没停够等停够、没有事件到点也换、按跑了多久分档、新的一轮从头挑；
//! 点跟着流光轮换；最宽的词；什么算出了事。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Pulse, widest, columns, dotCount, beatOf, localWords, retryLine } from '../../packages/pulse/model.js';
import { local } from '../../src/lib/text.js';

/** 这个包的设置项的出厂值 */
const config = Object.fromEntries(Object.entries(JSON.parse(readFileSync(new URL('../../packages/pulse/manifest.json', import.meta.url), 'utf8')).settings).map(([k, s]) => [k, s.default]));


/** 出厂的词库照中文、日文挑出来的 */
const zh = localWords(config.words, (v) => local(v, { code: 'zh', fallback: 'zh' }));
const ja = localWords(config.words, (v) => local(v, { code: 'ja', fallback: 'zh' }));

/** 范围的两头一样：测试里停多久是定的。 */
const WORDS = {
  dwell_ms: [12_000, 12_000],
  quiet_ms: 2000,
  idle_ms: [30_000, 30_000],
  tiers: [
    { after: 0, words: ['甲', '乙', '丙'] },
    { after: 60, words: ['久一', '久二'] },
  ],
};

/** 定死的随机数：一个小的线性同余，同样的种子出同样的一串。 */
function seeded(seed) {
  let s = seed;
  return () => {
    s = (s * 1103515245 + 12345) % 2147483648;
    return s / 2147483648;
  };
}

const T0 = 1_000_000;
const TURN = { id: 'S:3', start: T0 };

test('一阵事件：安静下来、这个词停够了才换，换的不是刚才那个', () => {
  const p = new Pulse(seeded(7));
  const first = p.word(TURN, 0, T0, WORDS);
  for (const [ms, beat] of [[300, 1], [900, 2], [1500, 3], [3000, 4]]) {
    assert.equal(p.word(TURN, beat, T0 + ms, WORDS), first, '事件还在来，不换');
  }
  assert.equal(p.word(TURN, 4, T0 + 5000, WORDS), first, '安静了 2 秒，但这个词才停了 5 秒：等');
  const next = p.word(TURN, 4, T0 + 12_000, WORDS);
  assert.notEqual(next, first, '停够了：换');
  assert.equal(p.shown, T0 + 12_000, '换上的时刻：流光从头扫');
  assert.equal(p.word(TURN, 4, T0 + 25_000, WORDS), next, '没有新事件、没到 30 秒：不换');
});

test('事件一直不停不闪：到了没事件时的时长才兜底换一个', () => {
  const p = new Pulse(seeded(7));
  const first = p.word(TURN, 0, T0, WORDS);
  for (let n = 1; n < 58; n++) assert.equal(p.word(TURN, n, T0 + n * 500, WORDS), first);
  assert.notEqual(p.word(TURN, 60, T0 + 30_000, WORDS), first);
});

test('按这一轮跑了多久分档；新的一轮回到第一档', () => {
  const p = new Pulse(seeded(7));
  p.word(TURN, 0, T0, WORDS);
  assert.ok(['久一', '久二'].includes(p.word(TURN, 0, T0 + 61_000, WORDS)), '跑过 60 秒：停够了就换到下一档');
  const next = { id: 'S:9', start: T0 + 70_000 };
  assert.ok(['甲', '乙', '丙'].includes(p.word(next, 0, T0 + 70_000, WORDS)), '新的一轮回到第一档');
});

test('只有一个词的档：照旧挑它，不卡住', () => {
  const p = new Pulse(seeded(3));
  const one = { ...WORDS, tiers: [{ after: 0, words: ['独'] }] };
  assert.equal(p.word(TURN, 0, T0, one), '独');
  assert.equal(p.word(TURN, 0, T0 + 40_000, one), '独');
});

test('停多久在范围里随机定：两头都取得到', () => {
  const p = new Pulse(() => 0.999_999);
  const w = { ...WORDS, dwell_ms: [12_000, 20_000] };
  const first = p.word(TURN, 0, T0, w);
  p.word(TURN, 1, T0 + 100, w);
  assert.equal(p.word(TURN, 1, T0 + 19_999, w), first, '取到上头：20 秒以前不换');
  assert.notEqual(p.word(TURN, 1, T0 + 20_000, w), first);
});

test('点跟着流光轮换：一趟亮光轮一圈，一个点停 1 秒', () => {
  const { sweep_seconds: sweep } = config;
  const { dot_count: count } = config;
  assert.equal(dotCount(0, sweep, count), 1);
  assert.equal(dotCount(sweep / 3 + 0.01, sweep, count), 2);
  assert.equal(dotCount((sweep * 2) / 3 + 0.01, sweep, count), 3);
  assert.equal(dotCount(sweep - 0.001, sweep, count), 3);
  assert.equal(dotCount(sweep + 0.01, sweep, count), 1);
  assert.equal(dotCount(5, 0, count), 1, '扫一趟是 0 秒的当没有');
});

test('最宽的词：中文一个字两格', () => {
  assert.equal(columns('深度求索'), 8);
  assert.equal(columns('ab'), 2);
  assert.equal(widest(WORDS), '久一');
  assert.equal(columns(widest(zh)), 8);
});

test('算一件事的只有这几样（照 TUI）：开了新的一步、想完、工具出了结果、开始写回答；接着写字不算', () => {
  const events = [{ seq: 1, kind: 'turn.started' }];
  const live = { turn: 3, seen: 3, blocks: [{ kind: 'reasoning', text: '想', done: false }] };
  const writing = { ...live, blocks: [{ ...live.blocks[0], text: '想了很多很多' }] };
  const thought = { ...live, blocks: [{ ...live.blocks[0], text: '想想', done: true }] };
  const tool = { ...live, blocks: [thought.blocks[0], { kind: 'tool_call', text: '', done: false }] };
  const result = [...events, { seq: 2, kind: 'message.assistant', body: { blocks: [{}, {}] } }, { seq: 3, kind: 'tool.result', body: {} }];
  assert.equal(beatOf(events, live), beatOf(events, writing), '接着写字不算');
  const beats = [beatOf(events, null), beatOf(events, live), beatOf(events, thought), beatOf(events, tool), beatOf(result, null)];
  assert.equal(new Set(beats).size, beats.length, `每一件都变：${beats.join(' | ')}`);
});

test('词库照 TUI 的原样：三档，停 12–20 秒，安静 2 秒，没事件 30–45 秒', () => {
  assert.deepEqual(config.words.dwell_ms, [12000, 20000]);
  assert.equal(config.words.quiet_ms, 2000);
  assert.deepEqual(config.words.idle_ms, [30000, 45000]);
  assert.deepEqual(config.words.tiers.map((t) => t.after), [0, 30, 90]);
  assert.equal(zh.tiers[0].words[0], '深度求索');
});

test('词库每一档按语言写：日文照 TUI 的原样；个人改成只写一串的照旧能用（蓝图「界面语言」）', () => {
  assert.equal(ja.tiers[0].words[0], '深く探索中');
  assert.deepEqual(ja.tiers.map((t) => t.after), [0, 30, 90]);
  const old = { ...WORDS, tiers: [{ after: 0, words: ['一串'] }] };
  assert.deepEqual(localWords(old, (v) => local(v, { code: 'ja', fallback: 'zh' })).tiers[0].words, ['一串']);
});

test('重试那一截：还在等的写还要等多久（一秒走一格）；等到了写在重试；换端点当场再来的照旧', () => {
  const r = { attempt: 1, limit: 5, message: 'HTTP 524:\n error code: 524', due: 10_000 };
  assert.deepEqual(retryLine(r, 6_500), { key: 'retry_wait', fields: { attempt: 1, limit: 5, message: 'HTTP 524: error code: 524', wait: '4s' } });
  assert.deepEqual(retryLine(r, 9_001).fields.wait, '1s');
  assert.deepEqual(retryLine({ ...r, due: 80_000 }, 0).fields.wait, '1m 20s');
  assert.deepEqual(retryLine(r, 10_000), { key: 'retry', fields: { attempt: 1, limit: 5, message: 'HTTP 524: error code: 524' } });
  assert.equal(retryLine({ ...r, failover: true }, 0).key, 'retry_failover');
  assert.equal(retryLine({ ...r, due: undefined }, 0).key, 'retry', '不知道等多久的：照旧');
  assert.equal(retryLine(null, 0), null);
  const manifest = JSON.parse(readFileSync(new URL('../../packages/pulse/manifest.json', import.meta.url), 'utf8'));
  assert.equal(local(manifest.text, { code: 'zh', fallback: 'zh' }).retry_wait, ' · {wait} 后重试 {attempt}/{limit}：{message}');
  assert.ok(manifest.text.ja.retry_wait.includes('{wait}'));
});
