// @ts-check
//! 正文里不是你说的、不挂在她头下的几样（蓝图 `web.md`「不是你说的话」「后台命令、子代理的回报」「压缩、清空」，收尾那一行的
//! 出错写法）：谁说的、回报那一行、压缩和清空那一行、手动压缩那一轮不另起收尾行、分块。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, ev } from './support.js';
import { project } from '../src/model/transcript.js';
import { failureText } from '../src/model/notes.js';
import { group } from '../src/model/group.js';
import { footer } from '../src/model/footer.js';

loadRes();

const me = { kind: 'person', account: 'admin' };
const CHILD = '0199a000-0000-7000-8000-000000000002';
const usage = { uncached: 800, cache_read: 100, cache_write: 0, output: 50 };

/** 一轮：你说一句，她调 shell 在后台跑一条命令、派一个子代理，说完结束。 */
function jobsLog() {
  return [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '去跑测试，再派个人查文档' }] }, me),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 1, 'message.assistant', 3, { seen: 3, blocks: [
      { type: 'tool_call', call_id: 'c1', name: 'shell', args: '{"command":"cargo test","background":true,"description":"跑测试"}' },
      { type: 'tool_call', call_id: 'c2', name: 'agent', args: '{"description":"查文档"}' },
    ] }),
    ev(5, 2, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'text', text: 'started j1' }], effects: [{ kind: 'job.started', job: 'j1', what: 'command', title: '跑测试' }] }),
    ev(6, 2, 'tool.result', 3, { call_id: 'c2', status: 'ok', blocks: [{ type: 'text', text: 'started j2' }], effects: [{ kind: 'job.started', job: 'j2', what: 'agent', title: '查文档', session: CHILD }] }),
    ev(7, 3, 'message.assistant', 3, { seen: 6, blocks: [{ type: 'text', text: '都派出去了。' }] }),
    ev(8, 3, 'turn.ended', 3, { reason: 'completed' }),
  ];
}

const notes = (items) => items.filter((it) => it.type === 'note').map((it) => `${it.tone}|${it.mark}|${it.text}`);

test('后台命令的回报：完成带用时，失败带退出码或信号，停掉的暗，她停的另说；不属于哪一轮；点开是命令和输出', () => {
  const log = [...jobsLog(),
    ev(9, 20, 'job.reported', undefined, { job: 'j1', reason: 'exited', exit_code: 0, duration_ms: 20000, output: `sha256:${'c'.repeat(64)}`, chars: 1200 }),
    ev(10, 21, 'job.reported', undefined, { job: 'j1', reason: 'exited', exit_code: 101, duration_ms: 1500 }),
    ev(11, 22, 'job.reported', undefined, { job: 'j1', reason: 'exited', signal: 9 }),
    ev(12, 23, 'job.reported', undefined, { job: 'j1', reason: 'stopped', by_model: true }),
    ev(13, 24, 'job.reported', undefined, { job: 'j9', reason: 'aborted' }),
    ev(14, 25, 'job.reported', undefined, { job: 'j1', reason: 'exploded' }),
  ];
  const items = project(log).items;
  assert.deepEqual(notes(items), [
    'good|●|后台命令完成 · 跑测试 · 20.0s',
    'error|✗|后台命令失败 · 跑测试 · 退出码 101',
    'error|✗|后台命令失败 · 跑测试 · 信号 9',
    'stopped|●|后台命令已停止 · 跑测试',
    'stopped|●|后台命令中断：核心退出过 · j9',
    'dim||后台命令 · 跑测试 · exploded',
  ]);
  const first = items.find((it) => it.type === 'note');
  assert.equal(first.turn, null);
  assert.deepEqual(first.detail, { kind: 'output', command: 'cargo test', hash: `sha256:${'c'.repeat(64)}`, chars: 1200 });
});

test('子代理的回报：交回报告的点开是正文，截过的记下；停掉、撤销照换', () => {
  const log = [...jobsLog(),
    ev(9, 30, 'child.reported', undefined, { job: 'j2', session: CHILD, reason: 'done', text: '文档在 docs/ 下', truncated: true }, { kind: 'session', id: CHILD }),
    ev(10, 31, 'child.reported', undefined, { job: 'j2', session: CHILD, reason: 'undone', text: '' }, { kind: 'session', id: CHILD }),
  ];
  const items = project(log).items;
  assert.deepEqual(notes(items), ['good|●|子代理交回报告 · 查文档', 'stopped|●|子代理随撤销停止 · 查文档']);
  assert.deepEqual(items.find((it) => it.type === 'note').detail, { kind: 'text', text: '文档在 docs/ 下', truncated: true });
});

test('回报叫醒她开的一轮：没有你的话，她的一块接在回报下面；回报自己一块，不挂在她的头下', () => {
  const log = [...jobsLog(),
    ev(9, 20, 'job.reported', undefined, { job: 'j1', reason: 'exited', exit_code: 0, duration_ms: 20000 }),
    ev(10, 20, 'turn.started', 10, { trigger: 9 }),
    ev(11, 21, 'message.assistant', 10, { seen: 10, blocks: [{ type: 'text', text: '测试过了。' }] }),
    ev(12, 21, 'turn.ended', 10, { reason: 'completed' }),
  ];
  const blocks = group(project(log).items);
  assert.deepEqual(blocks.map((b) => b.kind), ['user', 'her', 'note', 'her']);
  assert.equal(blocks[3].turn, 10);
});

test('谁说的：你的、别的账号的、子代理发给她的（标题照派它的那次）、别的 harness、平台上的人', () => {
  const log = [...jobsLog(),
    ev(9, 40, 'message.user', undefined, { blocks: [{ type: 'text', text: '我看看' }] }, { kind: 'person', account: 'alice' }),
    ev(10, 41, 'message.user', undefined, { blocks: [{ type: 'text', text: '查到一半，要继续吗' }] }, { kind: 'session', id: CHILD }),
    ev(11, 42, 'message.user', undefined, { blocks: [{ type: 'text', text: 'hi' }] }, { kind: 'harness', name: 'claude-code' }),
    ev(12, 43, 'message.user', undefined, { blocks: [{ type: 'text', text: '在吗' }] }, { kind: 'external', platform: 'qq' }),
  ];
  const users = project(log).items.filter((it) => it.type === 'user');
  assert.deepEqual(users.map((u) => u.speaker), [
    { kind: 'person', account: 'admin', name: 'admin' },
    { kind: 'person', account: 'alice', name: 'alice' },
    { kind: 'agent', account: null, name: '子代理 · 查文档' },
    { kind: 'harness', account: null, name: 'claude-code · 别的 agent' },
    { kind: 'external', account: null, name: '外部' },
  ]);
});

test('压缩、清空那一行：清空绿点「上下文已清空」，压缩附了要求的接上；自动压缩在她那一轮中间也自己一块', () => {
  const log = [...jobsLog(),
    ev(9, 50, 'turn.started', 9, {}),
    ev(10, 51, 'context.compacted', 9, { upto: 8, summary: '', trigger: 'clear' }),
    ev(11, 51, 'turn.ended', 9, { reason: 'completed' }),
  ];
  const items = project(log).items;
  assert.deepEqual(notes(items), ['good|●|上下文已清空']);
  assert.equal(items.filter((it) => it.type === 'done').length, 1, '清空那一轮不另起收尾行');
  assert.equal(items.filter((it) => it.turn === 9 && it.type !== 'note').length, 0, '也不画她的头');
});

test('手动压缩那一轮：不另起收尾行，用时和用量接在那一行后面；没压成的照出错那一行写', () => {
  const log = [...jobsLog(),
    ev(9, 60, 'turn.started', 9, {}),
    ev(10, 72, 'model.called', 9, { seen: 8, messages: 4, result: 'ok', compaction: true, usage, model: 'm', endpoint: 'e' }),
    ev(11, 72, 'context.compacted', 9, { upto: 8, summary: '摘要', trigger: 'manual', instructions: '重点保留数据库设计' }),
    ev(12, 72.5, 'turn.ended', 9, { reason: 'completed' }),
  ];
  assert.deepEqual(notes(project(log).items), ['good|●|上下文已压缩 · 要求：重点保留数据库设计 · 12.5s · 950(C11%)']);
  const failed = [...jobsLog(),
    ev(9, 60, 'turn.started', 9, {}),
    ev(10, 61, 'model.called', 9, { seen: 8, messages: 4, result: 'error', compaction: true, error: { class: 'other', message: 'boom', status: 500 } }),
    ev(11, 61, 'turn.ended', 9, { reason: 'error' }),
  ];
  const done = project(failed).items.at(-1);
  assert.equal(done.type, 'done');
  assert.equal(done.text, '出错了：boom');
});

test('出错那一句：402、404 加人话；内核自己查出来的写分类，有原话的接后面；没原话的写分类', () => {
  assert.equal(failureText({ class: 'other', message: 'no money', status: 402 }), '额度用完了：no money');
  assert.equal(failureText({ class: 'other', message: 'model not found', status: 404 }), '找不到，检查端点地址和模型名：model not found');
  assert.equal(failureText({ class: 'bad_stream', message: 'eof' }), '回复的流不对：eof');
  assert.equal(failureText({ class: 'empty_reply', message: '' }), '回复是空的');
  assert.equal(failureText({ class: 'auth', message: '' }), '认证失败');
});

test('清空以后框下面那一行的上下文清零，下一次请求再照实际的写', () => {
  const log = [...jobsLog(),
    ev(9, 7, 'model.called', 3, { seen: 6, messages: 3, result: 'ok', usage, model: 'm', endpoint: 'e' }),
    ev(10, 50, 'context.compacted', 11, { upto: 9, summary: '', trigger: 'clear' }),
  ];
  assert.equal(footer(log, {}).right.find((p) => p.key === 'context'), undefined);
  log.push(ev(11, 60, 'model.called', 12, { seen: 10, messages: 1, result: 'ok', usage: { ...usage, uncached: 10, cache_read: 0 }, model: 'm', endpoint: 'e' }));
  assert.equal(footer(log, {}).right.find((p) => p.key === 'context')?.text, '60');
});

test('子会话里派它的会话发来的：写「派它的会话」；都对不上的写「另一个会话」', () => {
  const PARENT = '0199a000-0000-7000-8000-000000000001';
  const log = [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false }, parent: PARENT, depth: 1 }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '去查文档' }] }, { kind: 'session', id: PARENT }),
    ev(3, 1, 'message.user', undefined, { blocks: [{ type: 'text', text: '?' }] }, { kind: 'session', id: '0199a000-0000-7000-8000-00000000000f' }),
  ];
  assert.deepEqual(project(log).items.filter((it) => it.type === 'user').map((u) => u.speaker.name), ['派它的会话', '另一个会话']);
});

test('打断的那一轮收尾：还有在跑的后台任务的，记着几个（后面接一句「后台还有 N 个在跑」）', () => {
  const log = [...jobsLog().slice(0, 7),
    ev(8, 3, 'turn.ended', 3, { reason: 'interrupted' }),
  ];
  const done = project(log).items.at(-1);
  assert.equal(done.type, 'done');
  assert.equal(done.jobs, 2);
  const after = project([...log, ev(9, 20, 'job.reported', undefined, { job: 'j1', reason: 'exited', exit_code: 0 }),
    ev(10, 21, 'child.reported', undefined, { job: 'j2', session: CHILD, reason: 'done', text: '' }, { kind: 'session', id: CHILD })]).items.find((it) => it.type === 'done');
  assert.equal(after.jobs, 0, '都结束了：不再写');
});

test('她正在回答时来的回报：上面那段时间线收起，回报接在她那一块里面、不另起头，接着的步另起一段排在回报下面', () => {
  const log = [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '跑一下' }] }, me),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 1, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'tool_call', call_id: 'c1', name: 'shell', args: '{"command":"sleep 1","background":true}' }] }),
    ev(5, 2, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'text', text: 'started j1' }], effects: [{ kind: 'job.started', job: 'j1', what: 'command', title: '睡一秒' }] }),
    ev(6, 3, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'tool_call', call_id: 'c2', name: 'shell', args: '{"command":"ls"}' }] }),
    ev(7, 4, 'job.reported', undefined, { job: 'j1', reason: 'exited', exit_code: 0, duration_ms: 1000 }),
    ev(8, 5, 'tool.result', 3, { call_id: 'c2', status: 'ok', blocks: [{ type: 'text', text: 'a b' }] }),
    ev(9, 6, 'message.assistant', 3, { seen: 7, blocks: [{ type: 'tool_call', call_id: 'c3', name: 'shell', args: '{"command":"pwd"}' }] }),
    ev(10, 7, 'tool.result', 3, { call_id: 'c3', status: 'ok', blocks: [{ type: 'text', text: '/x' }] }),
    ev(11, 8, 'message.assistant', 3, { seen: 7, blocks: [{ type: 'text', text: '好了。' }] }),
    ev(12, 8, 'turn.ended', 3, { reason: 'completed' }),
  ];
  const kinds = (list) => list.map((it) => (it.type === 'steps' ? `steps${it.finished ? '✓' : ''}` : it.type));
  // 回报刚来、这一轮还在进行：上面那段已经收起
  assert.deepEqual(kinds(project(log.slice(0, 7)).items).slice(0, 3), ['user', 'steps✓', 'note'], '回报一来，上面那段就收起');
  const items = project(log).items;
  assert.deepEqual(kinds(items).slice(0, 4), ['user', 'steps✓', 'note', 'steps✓'], '后面的步另起一段，排在回报下面');
  assert.deepEqual(group(project(log.slice(0, 7)).items).map((b) => b.kind), ['user', 'her'], '回报刚来、下一步还没来：也已经在她那一块里（不先画成单独一块再挪进去）');
  const blocks = group(items);
  assert.deepEqual(blocks.map((b) => b.kind), ['user', 'her'], '回报在她那一块里，头只画一次');
  assert.deepEqual(blocks[1].items.map((it) => it.type), ['steps', 'note', 'steps', 'reply', 'done']);
});

test('她正在回答时插进来的话（子代理发来的）：上面那段一听到就收起；接在它后面的她那一块不再画头像和名字', () => {
  const kid = { kind: 'session', id: CHILD };
  const log = [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '去挖一挖' }] }, me),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 1, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'tool_call', call_id: 'c1', name: 'read', args: '{"file_path":"a"}' }] }),
    ev(5, 2, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'text', text: 'a' }] }),
    ev(6, 3, 'message.user', undefined, { blocks: [{ type: 'text', text: '第 1 个读完' }] }, kid),
    ev(7, 4, 'message.assistant', 3, { seen: 6, blocks: [{ type: 'tool_call', call_id: 'c2', name: 'read', args: '{"file_path":"b"}' }] }),
    ev(8, 5, 'tool.result', 3, { call_id: 'c2', status: 'ok', blocks: [{ type: 'text', text: 'b' }] }),
    ev(9, 6, 'message.assistant', 3, { seen: 6, blocks: [{ type: 'text', text: '好了。' }] }),
    ev(10, 6, 'turn.ended', 3, { reason: 'completed' }),
  ];
  const kinds = (list) => list.map((it) => (it.type === 'steps' ? `steps${it.finished ? '✓' : ''}` : it.type));
  assert.deepEqual(kinds(project(log.slice(0, 7)).items).slice(0, 3), ['user', 'steps✓', 'user'], '听到那句话的时候，上面那段已经收起');
  const blocks = group(project(log).items);
  assert.deepEqual(blocks.map((b) => `${b.kind}${b.cont ? '+' : ''}`), ['user', 'her', 'user', 'her+'], '后一块接着她这一轮，不画头像');
});

test('不带回合编号的话，她正在回答时来的也照排着的算：那次请求没听到就不进正文，这一轮结束后是下一轮的开头', () => {
  const kid = { kind: 'session', id: CHILD };
  const log = [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '等子代理的消息' }] }, me),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 1, 'model.called', 3, { request: 1 }),
    // 请求发出去以后才到的（seen 够不着它）
    ev(5, 2, 'message.user', undefined, { blocks: [{ type: 'text', text: '第 5 条' }] }, kid),
    ev(6, 3, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'text', text: '等第 5 条。' }] }),
    ev(7, 3, 'turn.ended', 3, { reason: 'completed' }),
    ev(8, 4, 'turn.started', 8, { trigger: 5 }),
    ev(9, 5, 'message.assistant', 8, { seen: 8, blocks: [{ type: 'text', text: '收到第 5 条。' }] }),
    ev(10, 5, 'turn.ended', 8, { reason: 'completed' }),
  ];
  const texts = (items) => items.map((it) => (it.type === 'user' ? `你:${it.text}` : it.type === 'reply' ? `她:${it.text}` : it.type)).filter((x) => x.includes(':'));
  assert.deepEqual(texts(project(log.slice(0, 6)).items), ['你:等子代理的消息', '她:等第 5 条。'], '这一轮还在进行、没听到：排着，不进正文');
  assert.deepEqual(texts(project(log.slice(0, 7)).items), ['你:等子代理的消息', '她:等第 5 条。', '你:第 5 条'], '这一轮结束了还排着：接在她的回答下面（不画在上面）');
  assert.deepEqual(texts(project(log).items), ['你:等子代理的消息', '她:等第 5 条。', '你:第 5 条', '她:收到第 5 条。'], '下一轮的开头');
});
