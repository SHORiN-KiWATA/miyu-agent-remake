// @ts-check
//! 时间线上的字（蓝图 `web.md`「时间线」）：一步那一行照核心给的标题那一句（9-8），思考收着时的那一小段、在想时的窗口、命令写在
//! 下面的几行、点开的细节。收起那一行核心算好了（`entries.test.js`）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, ms } from './support.js';
import { res } from '../src/util/res.js';
import { row, peek, messagePeek, thinkingTail, commandLines, details } from '../src/model/words.js';

loadRes();

/** 造一步：没写的照做完了、没出错；`title` 是核心给的标题那一句。 */
function tool(name, args, more = {}) {
  return { key: name, kind: 'tool', name, args: JSON.stringify(args), parsed: args, state: 'done', status: 'ok', output: '', said: null, start: ms(0), end: ms(1), duration: null, title: null, ...more };
}
function thought(text, start = null, end = null, more = {}) {
  return { key: 'th', kind: 'thought', text, state: 'done', start, end, ...more };
}

test('思考：在想的写「正在思考」、整秒走表；想完「已思考」、用时一位小数；历史里不知道的不写用时', () => {
  const busy = row(thought('x', ms(3), null, { state: 'thinking' }));
  assert.deepEqual([busy.icon, busy.name, busy.timer, busy.took], ['atom', '正在思考', { since: ms(3), format: 'secs' }, null]);
  const done = row(thought('x', ms(0), ms(12.34)));
  assert.deepEqual([done.name, done.timer, done.took], ['已思考', null, '12.3s']);
  assert.equal(row(thought('x')).took, null);
});

test('执行命令：图标 square-terminal（带外框），名字、对象照核心的标题；用时照这一步的用时，在跑的走表', () => {
  const title = { name: '执行命令', object: '列目录' };
  const r = row(tool('shell', { command: 'ls -la', description: '列目录' }, { duration: 23, title }));
  assert.deepEqual([r.icon, r.name, r.subject, r.took, r.mono], ['square-terminal', '执行命令', '列目录', '23 ms', false]);
  assert.equal(row(tool('shell', { command: 'x' }, { duration: 1234, title })).took, '1.2 s');
  assert.equal(row(tool('shell', { command: 'x' }, { duration: 12_400, title })).took, '12 s');
  const running = row(tool('shell', { command: 'x' }, { state: 'running', status: null, start: ms(5), title }));
  assert.deepEqual([running.took, running.timer], [null, { since: ms(5), format: 'job' }]);
  // 被拒绝的没有用时
  assert.equal(row(tool('shell', { command: 'x' }, { status: 'denied', title })).took, null);
});

test('别的工具：名字、等宽的对象、结果那一句照核心的标题；核心没给标题的照显示名、工具名写，图标照工具', () => {
  const r = row(tool('read', { file_path: '/home/me/notes.md' }, { title: { name: '读取', object: '~/notes.md', said: '12 行' } }));
  assert.deepEqual([r.icon, r.name, r.subject, r.said, r.mono, r.took], ['file-text', '读取', '~/notes.md', '12 行', true, null]);
  assert.equal(row(tool('read', { file_path: '/tmp/x' }, { title: { name: '读取', object: '/tmp/x' } })).said, null, '没有结果那一句的不写');
  const unknown = row(tool('mystery_tool', { x: 1 }));
  assert.deepEqual([unknown.icon, unknown.name, unknown.subject], ['wrench', 'mystery_tool', null]);
  assert.equal(row(tool('read', {})).name, res.human.tools.read.name, '没给标题的照显示名');
  assert.equal(row(tool('grep', { pattern: 'todo' })).icon, 'search');
  assert.equal(row(tool('edit', { file_path: '/a' })).icon, 'square-pen');
});

test('出错的、被拒的：图标换成 circle-alert、整行红（被拒的写入不能看着像写成了，照 TUI）；打断的不算', () => {
  const r = row(tool('shell', { command: 'x' }, { status: 'error' }));
  assert.deepEqual([r.icon, r.failed], ['circle-alert', true]);
  const d = row(tool('write', { file_path: '/home/me/a.txt', content: 'x' }, { status: 'denied' }));
  assert.deepEqual([d.icon, d.failed], ['circle-alert', true]);
  assert.equal(row(tool('shell', { command: 'x' }, { status: 'cancelled' })).failed, false);
});

test('还在写参数：名字照核心的标题（「准备执行命令」这种），图标转圈，一位小数走表', () => {
  const r = row(tool('shell', null, { state: 'preparing', status: null, parsed: null, start: ms(2), title: { name: '准备执行命令' } }));
  assert.deepEqual([r.icon, r.name, r.timer], ['loader-circle', '准备执行命令', { since: ms(2), format: 'tenths' }]);
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

test('点开的细节：编辑、写入是差异卡片，做成了不写结果；没做成的接着结果', () => {
  const edit = details(tool('edit', { file_path: '/a.rs', edits: [{ old_string: 'a\n', new_string: 'b\n' }] }, { output: 'ok' }));
  assert.equal(edit.length, 1);
  assert.deepEqual([edit[0].kind, edit[0].op, edit[0].path, edit[0].diff.added, edit[0].diff.removed], ['diff', '修改', '/a.rs', 1, 1]);
  const failed = details(tool('edit', { file_path: '/a.rs', edits: [{ old_string: 'a', new_string: 'b' }] }, { status: 'error', output: '没读过' }));
  assert.deepEqual(failed.map((s) => s.kind === 'diff' ? 'diff' : s.label), ['diff', '结果']);
});

test('派子代理那一步：写「编号 · 描述」（对象是核心给的描述），不写结果那一句；还没派出去的只写描述；点开是完整的提示词', () => {
  const args = { description: 'Fix 2 mismatches', prompt: '你是子代理。\n先读 docs/，再改两处不一致。' };
  const title = { name: '派子代理', object: 'Fix 2 mismatches', said: '派出了 j2' };
  const spawned = tool('subagent', args, { job: 'j2', title, output: 'started j2' });
  const r = row(spawned);
  assert.deepEqual([r.icon, r.name, r.subject, r.said], ['bot', '派子代理', 'j2 · Fix 2 mismatches', null]);
  assert.deepEqual(details(spawned), [{ kind: 'text', label: '提示词', text: '你是子代理。\n先读 docs/，再改两处不一致。' }]);
  assert.equal(row(tool('subagent', args, { state: 'running', status: null, title })).subject, 'Fix 2 mismatches');
});

test('留言那一步：对象照核心写好的（任务编号、父会话、会话 短编号），送到了不写结果那一句；收着时后面是留言开头的预览，点开是发给谁、完整的消息', () => {
  const text = '先别改 a.rs，\n  我这边刚发现它被别处引用了。';
  const sent = tool('send_message', { to: 'j2', message: text }, { toTitle: 'Fix 2 mismatches', title: { name: '留言', object: 'j2' }, output: 'Message sent to j2.' });
  const r = row(sent);
  assert.deepEqual([r.icon, r.subject, r.mono, r.said], ['bot-message-square', 'j2', false, null]);
  assert.equal(messagePeek(sent), '先别改 a.rs， 我这边刚发现它被别处引用了。', '空白压成一个空格');
  assert.deepEqual(details(sent), [
    { kind: 'text', label: '发给', text: 'j2 · Fix 2 mismatches' },
    { kind: 'text', label: '消息', text },
  ]);
  // 点开的「发给」照参数写：找不到标题的只写编号；父会话写「父会话」；会话编号整个的、后缀的都写最后 8 位
  assert.equal(details(tool('send_message', { to: 'j3', message: 'x' }))[0].text, 'j3');
  assert.equal(details(tool('send_message', { to: 'parent', message: 'x' }))[0].text, '父会话');
  assert.equal(details(tool('send_message', { to: '019a6f2e-7c41-7d3b-9a52-1f0e8c3b4d5a', message: 'x' }))[0].text, '会话 8c3b4d5a');
  assert.equal(details(tool('send_message', { to: 'j12', message: 'x' }))[0].text, 'j12', '任务编号不是会话');
  // 没送到、存下了的照写结果那一句，点开接着结果
  const stopped = tool('send_message', { to: 'j2', message: 'x' }, { status: 'error', title: { name: '留言', object: 'j2', said: 'j2 已经停了' }, output: 'Subagent j2 was stopped.' });
  assert.equal(row(stopped).said, 'j2 已经停了');
  assert.equal(details(stopped).at(-1)?.label, '结果');
  const held = tool('send_message', { to: '8c3b4d5a', message: 'x' }, { title: { name: '留言', object: '会话 8c3b4d5a', said: '给 8c3b4d5a 存下了' }, output: 'Saved.' });
  assert.equal(row(held).said, '给 8c3b4d5a 存下了');
  assert.equal(details(held).at(-1)?.label, '结果');
});

test('留言的预览：长的截开头 peek_chars 个字、末尾写 …；别的步没有', () => {
  const long = 'a'.repeat(400);
  const p = messagePeek(tool('send_message', { to: 'j2', message: long }));
  assert.equal(p.length, 160);
  assert.ok(p.endsWith('…'));
  assert.equal(messagePeek(tool('read', { file_path: 'x' })), '');
  assert.equal(messagePeek(thought('想')), '');
});

test('每件工具都有自己的图标，不落到扳手：列会话 sessions 和左栏「会话」同一个（2026-10-01 项目主人指出）', () => {
  assert.equal(row(tool('sessions', {})).icon, 'message-circle');
  for (const name of ['shell', 'read', 'glob', 'grep', 'history', 'write', 'edit', 'trash', 'subagent', 'send_message', 'jobs', 'sessions', 'ask_user', 'todowrite', 'session_usage']) {
    assert.notEqual(row(tool(name, {})).icon, res.timeline.icon_default, name);
  }
});

test('编辑、写入那一行：后面接这一步加减的行数（核心给的，没给的照参数）；加减都是 0、没改成的不写（2026-10-01）', () => {
  const write = tool('write', { file_path: '/tmp/a.txt', content: '一行\n' });
  assert.deepEqual(row(write).diff, { added: 1, removed: 0 });
  assert.deepEqual(row(tool('write', write.parsed, { lines: { added: 7, removed: 2 } })).diff, { added: 7, removed: 2 }, '核心给的真数');
  const edit = tool('edit', { file_path: '/tmp/a.txt', edits: [{ old_string: 'a\nb\n', new_string: 'a\nc\nd\n' }] });
  assert.deepEqual(row(edit).diff, { added: 2, removed: 1 });
  assert.deepEqual(row(tool('edit', edit.parsed, { state: 'running', status: null })).diff, { added: 2, removed: 1 }, '在跑的照参数写');
  assert.equal(row(tool('edit', edit.parsed, { status: 'error' })).diff, null, '出错的不写');
  assert.equal(row(tool('edit', edit.parsed, { status: 'cancelled' })).diff, null, '打断的不写');
  assert.equal(row(tool('edit', { file_path: '/tmp/a.txt', edits: [{ old_string: 'x', new_string: 'x' }] })).diff, null, '加减都是 0');
  assert.equal(row(tool('read', { file_path: '/tmp/a.txt' })).diff, null, '不是编辑');
});
