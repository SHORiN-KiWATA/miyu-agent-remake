// @ts-check
//! 时间线上的字（蓝图 `web.md`「时间线」，照旧版网页）：一步那一行、思考收着时的那一小段和在想时的窗口、命令写在
//! 下面的几行、点开的细节；收起那一行照 TUI（`tui.md`「时间线」第 17 条）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, ms } from './support.js';
import { readFileSync } from 'node:fs';
import { res } from '../src/util/res.js';
import { row, peek, messagePeek, thinkingTail, commandLines, details, summary } from '../src/model/words.js';

loadRes();

const HOME = '/home/me';

/** 造一步：没写的照做完了、没出错。 */
function tool(name, args, more = {}) {
  return { key: name, kind: 'tool', name, args: JSON.stringify(args), parsed: args, state: 'done', status: 'ok', output: '', said: null, start: ms(0), end: ms(1), duration: null, ...more };
}
/**
 * 临时放几句说法再跑：核心的说法是仓库资源目录里读的，有的要合进 main 才有；跑完放回原样。
 * @param {Record<string, string>} templates
 * @param {() => void} fn
 */
function withSaid(templates, fn) {
  const said = /** @type {Record<string, string>} */ (res.human.said);
  const had = Object.fromEntries(Object.keys(templates).map((k) => [k, said[k]]));
  Object.assign(said, templates);
  try {
    fn();
  } finally {
    for (const [k, v] of Object.entries(had)) if (v === undefined) delete said[k]; else said[k] = v;
  }
}
function thought(text, start = null, end = null, more = {}) {
  return { key: 'th', kind: 'thought', text, state: 'done', start, end, ...more };
}

/** 收起那一行写成一行字。 */
const line = (steps) => summary(steps, ms(100)).spans.map((s) => s.text).join('');

test('思考：在想的写「正在思考」、整秒走表；想完「已思考」、用时一位小数；历史里不知道的不写用时', () => {
  const busy = row(thought('x', ms(3), null, { state: 'thinking' }), HOME);
  assert.deepEqual([busy.icon, busy.name, busy.timer, busy.took], ['atom', '正在思考', { since: ms(3), format: 'secs' }, null]);
  const done = row(thought('x', ms(0), ms(12.34)), HOME);
  assert.deepEqual([done.name, done.timer, done.took], ['已思考', null, '12.3s']);
  assert.equal(row(thought('x'), HOME).took, null);
});

test('执行命令：图标 square-terminal（带外框），对象是短标题；用时照 duration_ms，在跑的走表', () => {
  const r = row(tool('shell', { command: 'ls -la', description: '列目录' }, { duration: 23 }), HOME);
  assert.deepEqual([r.icon, r.name, r.subject, r.took, r.mono], ['square-terminal', '执行命令', '列目录', '23 ms', false]);
  assert.equal(row(tool('shell', { command: 'x' }, { duration: 1234 }), HOME).took, '1.2 s');
  assert.equal(row(tool('shell', { command: 'x' }, { duration: 12_400 }), HOME).took, '12 s');
  const running = row(tool('shell', { command: 'x' }, { state: 'running', status: null, start: ms(5) }), HOME);
  assert.deepEqual([running.took, running.timer], [null, { since: ms(5), format: 'job' }]);
  // 被拒绝的没有用时
  assert.equal(row(tool('shell', { command: 'x' }, { status: 'denied' }), HOME).took, null);
});

test('别的工具：显示名、等宽的对象（家目录写成 ~）、结果那一句；没登记的写工具名、图标 wrench', () => {
  const said = { key: 'software/basesystem/read/lines', fields: { count: '12' } };
  const r = row(tool('read', { file_path: '/home/me/notes.md' }, { said }), HOME);
  assert.deepEqual([r.icon, r.name, r.subject, r.said, r.mono, r.took], ['file-text', '读取', '~/notes.md', '12 行', true, null]);
  // 结果那一句少了字段的不写
  const bad = { key: 'software/basesystem/read/lines', fields: {} };
  assert.equal(row(tool('read', { file_path: '/tmp/x' }, { said: bad }), HOME).said, null);
  const unknown = row(tool('mystery_tool', { x: 1 }), HOME);
  assert.deepEqual([unknown.icon, unknown.name, unknown.subject], ['wrench', 'mystery_tool', null]);
  assert.equal(row(tool('grep', { pattern: 'todo' }), HOME).icon, 'search');
  assert.equal(row(tool('edit', { file_path: '/a' }), HOME).icon, 'square-pen');
});

test('出错的、被拒的：图标换成 circle-alert、整行红（被拒的写入不能看着像写成了，照 TUI）；打断的不算', () => {
  const r = row(tool('shell', { command: 'x' }, { status: 'error' }), HOME);
  assert.deepEqual([r.icon, r.failed], ['circle-alert', true]);
  const d = row(tool('write', { file_path: '/home/me/a.txt', content: 'x' }, { status: 'denied' }), HOME);
  assert.deepEqual([d.icon, d.failed], ['circle-alert', true]);
  assert.equal(row(tool('shell', { command: 'x' }, { status: 'cancelled' }), HOME).failed, false);
});

test('还在写参数：照种类写「准备执行」「准备编辑」「准备工具」，一位小数走表', () => {
  const r = row(tool('shell', null, { state: 'preparing', parsed: null, start: ms(2) }), HOME);
  assert.deepEqual([r.icon, r.name, r.timer], ['loader-circle', '准备执行', { since: ms(2), format: 'tenths' }]);
  assert.equal(row(tool('write', null, { state: 'preparing', parsed: null }), HOME).name, '准备编辑');
  assert.equal(row(tool('read', null, { state: 'preparing', parsed: null }), HOME).name, '准备调用工具');
});

test('思考收着时的那一小段：最后 160 个字，空白压成一个空格，截了的打头写 …、不留半个英文词；在想时的窗口是最后 10 行', () => {
  const text = `开头  很长\n\n${'字'.repeat(200)}  结尾`;
  const p = peek(thought(text));
  assert.equal(p.length, 160);
  assert.ok(p.startsWith('…字') && p.endsWith('字 结尾'), '截掉了前面的打头写 …');
  assert.equal(peek(thought('  \n ')), '');
  assert.equal(peek(thought('想  好了')), '想 好了', '没截的不加 …');
  const english = peek(thought(`abc what${' y'.repeat(78)}`));
  assert.equal(english, `…${' y'.repeat(78).trimStart()}`, '截在 what 中间：留下的 hat 不要');
  const lines = Array.from({ length: 14 }, (_, i) => `第${i + 1}行`).join('\n');
  assert.deepEqual(thinkingTail(`\n${lines}\n`).split('\n'), Array.from({ length: 10 }, (_, i) => `第${i + 5}行`));
  assert.deepEqual(thinkingTail(lines, 3).split('\n'), ['第12行', '第13行', '第14行'], '要几行给几行');
});

test('命令写在下面的几行：最多 8 行，放不下时让出最后一行写 ⋮；还没有命令的没有', () => {
  const eight = Array.from({ length: 8 }, (_, i) => `c${i}`).join('\n');
  assert.deepEqual(commandLines(tool('shell', { command: eight })), { lines: eight.split('\n'), more: false });
  const nine = `${eight}\nc8`;
  assert.deepEqual(commandLines(tool('shell', { command: nine })), { lines: eight.split('\n').slice(0, 7), more: true });
  assert.equal(commandLines(tool('shell', {}, { state: 'preparing', parsed: null })), null);
  assert.equal(commandLines(tool('read', { file_path: '/a' })), null);
});

test('点开的细节：参数一行一个「键: 值」，再是结果；没有的段不写', () => {
  const d = details(tool('read', { file_path: '/a', limit: 30, env: { A: 1 } }, { output: 'a\nb\n' }));
  assert.deepEqual(d, [
    { kind: 'text', label: '参数', text: 'file_path: /a\nlimit: 30\nenv: {"A":1}' },
    { kind: 'text', label: '结果', text: 'a\nb' },
  ]);
  assert.deepEqual(details(tool('read', {}, { output: '' })), []);
});

test('读图的结果：点开时「结果」下面是那几张图（照 blob 取，蓝图「图片」第 2 条）；没有字的也有「结果」', () => {
  const images = [{ blob: `sha256:${'a'.repeat(64)}`, media_type: 'image/png', width: 800, height: 600 }];
  const d = details(tool('read', { file_path: '/tmp/a.png' }, { output: '', images }));
  assert.deepEqual(d, [
    { kind: 'text', label: '参数', text: 'file_path: /tmp/a.png' },
    { kind: 'images', label: '结果', images },
  ]);
});

test('点开执行命令：命令那几行留着，参数里不再写 command、description；命令被截了的才把全文写进参数', () => {
  const short = details(tool('shell', { command: 'ls', description: '列目录', timeout: 30 }, { output: 'a\n' }));
  assert.deepEqual(short, [
    { kind: 'text', label: '参数', text: 'timeout: 30' },
    { kind: 'text', label: '结果', text: 'a' },
  ]);
  assert.deepEqual(details(tool('shell', { command: 'ls', description: '列目录' }, { output: 'a' })).map((s) => s.label), ['结果']);
  const long = Array.from({ length: 12 }, (_, i) => `echo ${i}`).join('\n');
  const cut = details(tool('shell', { command: long, description: '很长' }, { output: '' }));
  assert.deepEqual(cut, [{ kind: 'text', label: '参数', text: `command: ${long}` }]);
});

test('点开的细节：编辑、写入是差异卡片，做成了不写结果；新建的照结果那一句', () => {
  const edit = details(tool('edit', { file_path: '/a.rs', edits: [{ old_string: 'a\n', new_string: 'b\n' }] }, { output: 'ok' }));
  assert.equal(edit.length, 1);
  assert.deepEqual([edit[0].kind, edit[0].op, edit[0].path, edit[0].diff.added, edit[0].diff.removed], ['diff', '修改', '/a.rs', 1, 1]);
  const created = details(tool('write', { file_path: '/b.md', content: 'x\n' }, { said: { key: 'software/basesystem/write/created', fields: { count: '1' } } }));
  assert.equal(created[0].op, '新建');
  const failed = details(tool('edit', { file_path: '/a.rs', edits: [{ old_string: 'a', new_string: 'b' }] }, { status: 'error', output: '没读过' }));
  assert.deepEqual(failed.map((s) => s.kind === 'diff' ? 'diff' : s.label), ['diff', '结果']);
});

test('收起那一行：只想过的写 Thought for；历史里不知道时长的写几段思考', () => {
  assert.equal(line([thought('a', ms(0), ms(26.4))]), 'Thought for 26s');
  assert.equal(line([thought('a', ms(0), ms(0.3))]), 'Thought for 1s');
  assert.equal(line([thought('a'), thought('b')]), '2 thoughts');
});

test('收起那一行：只有一条命令的写它的短标题，出错的整行红', () => {
  const steps = [thought('a', ms(0), ms(1)), tool('shell', { command: 'ls', description: 'List target project dirs' }, { start: ms(1), end: ms(3) })];
  assert.equal(line(steps), 'List target project dirs · 1 thought · 3s');
  const failed = [tool('shell', { command: 'ls', description: 'List' }, { status: 'error' })];
  assert.equal(summary(failed, ms(100)).failed, true);
  assert.equal(line(failed), 'List · 1 err · 1s');
});

test('收起那一行：按类数，打头的类不再写一遍；编辑那一格接加减的总行数', () => {
  const edit = tool('edit', { file_path: '/a', edits: [{ old_string: 'a\n', new_string: 'b\nc\nd\n' }] });
  const steps = [
    tool('shell', { command: 'a' }, { start: ms(0), end: ms(10) }),
    tool('shell', { command: 'b' }),
    edit,
    thought('x', ms(2), ms(3)), thought('y', ms(4), ms(5)), thought('z', ms(6), ms(48)),
  ];
  assert.equal(line(steps), 'Ran 2 commands · 1 edit +3 -1 · 3 thoughts · 48s');
  const spans = summary(steps, ms(100)).spans;
  assert.deepEqual(spans.filter((s) => s.tone !== 'base').map((s) => [s.text, s.tone]), [['+3', 'added'], ['-1', 'removed']]);
  assert.equal(line([tool('read', {}), tool('grep', {}, { end: ms(5) }), thought('x', ms(0), ms(1))]), 'Used 2 tools · 1 thought · 5s');
  assert.equal(line([edit, { ...edit, key: 'e2' }]), 'Made 2 edits +6 -2 · 1s');
  assert.equal(line([tool('shell', { command: 'a' }), tool('read', {}, { end: ms(65) })]), 'Ran 1 command · 1 tool · 1m 05s');
});

test('收起那一行：一步的时刻都不知道的不写用时', () => {
  assert.equal(line([tool('read', {}, { start: null, end: null }), thought('x')]), 'Used 1 tool · 1 thought');
});

test('收起那一行：派子代理写 Spawned，给子代理留言写 Messaged，不写 Used 1 tool', () => {
  const agent = tool('subagent', { description: '查文档', prompt: 'x' });
  const message = tool('send_message', { to: 'j2', message: 'x' });
  assert.equal(line([agent]), 'Spawned 1 agent · 1s');
  assert.equal(line([agent, agent, message]), 'Spawned 2 agents · 1 message · 1s');
  assert.equal(line([message]), 'Messaged 1 agent · 1s');
  assert.equal(line([tool('shell', { command: 'ls', description: 'List' }), agent]), 'List · 1 agent · 1s');
});

test('收起那一行：一段里只有一条命令、有短标题时，和编辑、别的工具同段也用短标题打头（2026-10-02 项目主人定，和 TUI 一样）', () => {
  const edit = tool('edit', { file_path: '/a', edits: [{ old_string: 'a\n', new_string: 'b\nc\nd\n' }] });
  assert.equal(line([tool('shell', { command: 'ls', description: 'List' }), edit]), 'List · 1 edit +3 -1 · 1s');
  assert.equal(line([tool('shell', { command: 'ls', description: 'List' }), tool('read', {})]), 'List · 1 tool · 1s');
  // 短标题认准命令那一步：同段的子代理也带 `description`，不取它
  assert.equal(line([tool('subagent', { description: '查文档', prompt: 'x' }), tool('shell', { command: 'ls', description: 'List' })]), 'List · 1 agent · 1s');
  // 命令没有短标题、或者不止一条命令的照旧按类数
  assert.equal(line([tool('shell', { command: 'ls' }), edit]), 'Ran 1 command · 1 edit +3 -1 · 1s');
  assert.equal(line([tool('shell', { command: 'a', description: '一' }), tool('shell', { command: 'b', description: '二' })]), 'Ran 2 commands · 1s');
  // 和别的工具同段、命令出错的：不整行红，`err` 那一格照旧红
  const failed = [tool('shell', { command: 'ls', description: 'List' }, { status: 'error' }), edit];
  assert.equal(summary(failed, ms(100)).failed, false);
  assert.equal(line(failed), 'List · 1 edit +3 -1 · 1 err · 1s');
});

test('派子代理那一步：写「派子代理 · 编号 · 标题」（编号照结果里的 job.started），不写结果那一句；点开是完整的提示词', () => {
  const args = { description: 'Fix 2 mismatches', prompt: '你是子代理。\n先读 docs/，再改两处不一致。' };
  const spawned = tool('subagent', args, { state: 'done', status: 'ok', job: 'j2', said: { key: 'agent/started', fields: { job: 'j2' } }, output: 'started j2' });
  const r = row(spawned, HOME);
  assert.equal(r.name, '派子代理');
  assert.equal(r.subject, 'j2 · Fix 2 mismatches');
  assert.equal(r.said, null);
  assert.deepEqual(details(spawned), [{ kind: 'text', label: '提示词', text: '你是子代理。\n先读 docs/，再改两处不一致。' }]);
  const running = tool('subagent', args, { state: 'running' });
  assert.equal(row(running, HOME).subject, 'Fix 2 mismatches', '还没派出去（没有编号）的只写标题');
});

test('留言那一步：写「留言 · j2」，送到了不接结果那一句；收着时后面是留言开头的预览，点开是发给谁、完整的消息（2026-10-01）', () => {
  const text = '先别改 a.rs，\n  我这边刚发现它被别处引用了。';
  const sent = tool('send_message', { to: 'j2', message: text }, { toTitle: 'Fix 2 mismatches', said: { key: 'software/basesystem/send_message/sent', fields: { to: 'j2' } }, output: 'Message sent to j2.' });
  const r = row(sent, HOME);
  assert.equal(r.subject, 'j2');
  assert.equal(r.mono, false);
  assert.equal(r.said, null, '送到了那一句和对象重了，不写');
  assert.equal(messagePeek(sent), '先别改 a.rs， 我这边刚发现它被别处引用了。', '空白压成一个空格');
  assert.deepEqual(details(sent), [
    { kind: 'text', label: '发给', text: 'j2 · Fix 2 mismatches' },
    { kind: 'text', label: '消息', text },
  ]);
  // 找不到标题的只写编号；发给父会话的写「父会话」
  assert.equal(details(tool('send_message', { to: 'j3', message: 'x' }))[0].text, 'j3');
  const up = tool('send_message', { to: 'parent', message: '做完一半了' });
  assert.equal(row(up, HOME).subject, '父会话');
  assert.equal(details(up)[0].text, '父会话');
  // 没送到的照写结果那一句，点开接着结果
  const key = 'software/basesystem/send_message/stopped';
  withSaid({ [key]: '{to} 已经停了' }, () => {
    const stopped = tool('send_message', { to: 'j2', message: 'x' }, { status: 'error', said: { key, fields: { to: 'j2' } }, output: 'Subagent j2 was stopped and takes no more messages.' });
    assert.equal(row(stopped, HOME).said, 'j2 已经停了');
    assert.equal(details(stopped).at(-1)?.label, '结果');
  });
});

test('留言发给别的会话（核心施工 C-5）：写「会话 短编号」；存下了照写那一句', () => {
  const sent = tool('send_message', { to: 'j2', message: '先别改' }, { toTitle: 'Fix 2 mismatches' });
  // 显示名是核心的工具表给的（合进 main 以后有 send_message），这里只看网页这边：图标、算留言、对象、预览、细节
  assert.equal(row(sent, HOME).icon, 'bot-message-square');
  assert.equal(messagePeek(sent), '先别改');
  assert.equal(details(sent)[0].text, 'j2 · Fix 2 mismatches');
  // 会话编号：整个的、后缀的都写最后 8 位（核心的短编号）
  const peer = tool('send_message', { to: '019a6f2e-7c41-7d3b-9a52-1f0e8c3b4d5a', message: '帮我看下' });
  assert.equal(row(peer, HOME).subject, '会话 8c3b4d5a');
  assert.equal(details(peer)[0].text, '会话 8c3b4d5a');
  assert.equal(row(tool('send_message', { to: '0e8c3b4d5a', message: 'x' }), HOME).subject, '会话 8c3b4d5a');
  assert.equal(row(tool('send_message', { to: 'j12', message: 'x' }), HOME).subject, 'j12', '任务编号不是会话');
  // 存下了（对方是没人看着的一次性会话）：送到了以外的话都照写
  const key = 'software/basesystem/send_message/held';
  withSaid({ [key]: '给 {to} 存下了' }, () => {
    const held = tool('send_message', { to: '8c3b4d5a', message: 'x' }, { said: { key, fields: { to: '8c3b4d5a' } }, output: 'Message saved for 8c3b4d5a.' });
    assert.equal(row(held, HOME).said, '给 8c3b4d5a 存下了');
    assert.equal(details(held).at(-1)?.label, '结果');
  });
  // 只订「空了告诉我」、不带话的（C-6）：那一句照写
  const watching = 'software/basesystem/send_message/watching';
  withSaid({ [watching]: '{to} 空了会告诉你' }, () => {
    const watch = tool('send_message', { to: '8c3b4d5a', notify_when_idle: true }, { said: { key: watching, fields: { to: '8c3b4d5a' } }, output: 'Watching.' });
    assert.equal(row(watch, HOME).said, '8c3b4d5a 空了会告诉你');
  });
});

test('留言的预览：长的截开头 peek_chars 个字、末尾写 …；别的步没有', () => {
  const long = 'a'.repeat(400);
  const p = messagePeek(tool('send_message', { to: 'j2', message: long }));
  assert.equal(p.length, 160);
  assert.ok(p.endsWith('…'));
  assert.equal(messagePeek(tool('read', { file_path: 'x' })), '');
  assert.equal(messagePeek(thought('想')), '');
});

test('收起那一行：手动定了语言的照那种语言写（中文、日文），跟着浏览器的是英文（2026-10-01 项目主人定）', () => {
  const english = res.text.timeline.summary;
  const own = (code) => JSON.parse(readFileSync(new URL(`../resources/text/${code}.json`, import.meta.url), 'utf8')).timeline.summary;
  const steps = [tool('shell', { command: 'a' }), tool('shell', { command: 'b' }), tool('edit', {}), thought('x', ms(0), ms(2))];
  const agent = tool('subagent', { description: '查文档', prompt: 'x' });
  try {
    res.text.timeline.summary = own('zh');
    assert.equal(line(steps), '执行了 2 条命令 · 1 处编辑 · 1 次思考 · 2s');
    assert.equal(line([agent, tool('send_message', { to: 'j2', message: 'x' })]), '派了 1 个子代理 · 1 条留言 · 1s');
    assert.equal(line([thought('x', ms(0), ms(26))]), '思考了 26s');
    res.text.timeline.summary = own('ja');
    assert.equal(line(steps), 'コマンドを 2 件実行 · 編集 1 件 · 思考 1 回 · 2s');
    assert.equal(line([agent]), 'サブエージェントを 1 件派遣 · 1s');
    assert.equal(line([thought('x', ms(0), ms(26))]), '思考時間 26s');
  } finally {
    res.text.timeline.summary = english;
  }
});

test('每件工具都有自己的图标，不落到扳手：列会话 sessions 和左栏「会话」同一个（2026-10-01 项目主人指出）', () => {
  assert.equal(row(tool('sessions', {}), HOME).icon, 'message-circle');
  for (const name of ['shell', 'read', 'glob', 'grep', 'history', 'write', 'edit', 'trash', 'subagent', 'send_message', 'jobs', 'sessions', 'ask_user', 'todowrite', 'session_usage']) {
    assert.notEqual(row(tool(name, {}), HOME).icon, res.timeline.icon_default, name);
  }
});

test('收起那一行：发给别的会话的留言另数一格，打不打头都写 Messaged N session(s)，排在给子代理的留言后面（2026-10-01 项目主人定，和 TUI 一样）', () => {
  const peer = (to) => tool('send_message', { to, message: 'x' });
  const id = '0199a000-0000-7000-8000-00000000000f';
  assert.equal(line([peer(id)]), 'Messaged 1 session · 1s');
  assert.equal(line([peer(id), peer('0000000f')]), 'Messaged 2 sessions · 1s');
  assert.equal(line([peer('j2'), peer(id)]), 'Messaged 1 agent · Messaged 1 session · 1s');
  assert.equal(line([tool('shell', { command: 'a' }), peer(id)]), 'Ran 1 command · Messaged 1 session · 1s');
  assert.equal(line([peer(id), tool('read', { file_path: 'x' })]), 'Messaged 1 session · 1 tool · 1s');
  assert.equal(line([peer('j10'), peer('parent')]), 'Messaged 2 agents · 1s', 'j10、parent 不是会话');
  const english = res.text.timeline.summary;
  const own = (code) => JSON.parse(readFileSync(new URL(`../resources/text/${code}.json`, import.meta.url), 'utf8')).timeline.summary;
  try {
    res.text.timeline.summary = own('zh');
    assert.equal(line([peer('j2'), peer(id)]), '给 1 个子代理留言 · 给 1 个会话留言 · 1s');
    res.text.timeline.summary = own('ja');
    assert.equal(line([peer(id)]), 'セッション 1 件にメッセージ · 1s');
  } finally {
    res.text.timeline.summary = english;
  }
});

test('编辑、写入那一行：结果那一句后面接这一步加减的行数（和收起那一行同一份）；加减都是 0、没改成的不写（2026-10-01）', () => {
  const write = tool('write', { file_path: '/tmp/a.txt', content: '一行\n' }, { said: { key: 'software/basesystem/write/created', fields: { lines: '1' } } });
  assert.deepEqual(row(write, HOME).diff, { added: 1, removed: 0 });
  const edit = tool('edit', { file_path: '/tmp/a.txt', edits: [{ old_string: 'a\nb\n', new_string: 'a\nc\nd\n' }] });
  assert.deepEqual(row(edit, HOME).diff, { added: 2, removed: 1 });
  assert.deepEqual(row(tool('edit', edit.parsed, { state: 'running', status: null }), HOME).diff, { added: 2, removed: 1 }, '在跑的照参数写');
  assert.equal(row(tool('edit', edit.parsed, { status: 'error' }), HOME).diff, null, '出错的不写');
  assert.equal(row(tool('edit', edit.parsed, { status: 'cancelled' }), HOME).diff, null, '打断的不写');
  assert.equal(row(tool('edit', { file_path: '/tmp/a.txt', edits: [{ old_string: 'x', new_string: 'x' }] }), HOME).diff, null, '加减都是 0');
  assert.equal(row(tool('read', { file_path: '/tmp/a.txt' }), HOME).diff, null, '不是编辑');
});

test('收起那一行：没改成的编辑（出错、被拒、打断）不算 edit、不算加减，算成用过一件工具，出错、被拒的再算一个 err（照 TUI）', () => {
  const ok = tool('write', { file_path: '/tmp/a.txt', content: '一行\n' });
  const denied = tool('write', { file_path: '/home/me/b.txt', content: '一行\n' }, { status: 'denied' });
  const broken = tool('edit', { file_path: '/tmp/a.txt', edits: [{ old_string: 'a', new_string: 'b' }] }, { status: 'error' });
  const cut = tool('edit', { file_path: '/tmp/a.txt', edits: [{ old_string: 'a', new_string: 'b' }] }, { status: 'cancelled' });
  assert.equal(line([ok, denied]), 'Used 1 tool · 1 edit +1 -0 · 1 err · 1s');
  assert.equal(line([ok, broken]), 'Used 1 tool · 1 edit +1 -0 · 1 err · 1s');
  assert.equal(line([ok, cut]), 'Used 1 tool · 1 edit +1 -0 · 1s');
  assert.equal(line([denied]), 'Used 1 tool · 1 err · 1s');
});

test('派子代理的工具 subagent：图标、那一行、收起那一行', () => {
  const args = { description: '查文档', prompt: '去查' };
  const neu = tool('subagent', args, { job: 'j3' });
  assert.equal(row(neu, HOME).icon, 'bot');
  assert.equal(row(neu, HOME).subject, 'j3 · 查文档');
  assert.equal(line([neu]), 'Spawned 1 agent · 1s');
  assert.deepEqual(details(neu), [{ kind: 'text', label: '提示词', text: '去查' }]);
});
