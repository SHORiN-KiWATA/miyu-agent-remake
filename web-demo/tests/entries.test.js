// @ts-check
//! 条目 → 正文的几种（蓝图 `web.md`「照条目画」第 2 条）：你的话（开轮那一句、排着的、撤回的）、她的回答、时间线的一段（步、收起那一行）、
//! 收尾那一行、旁白、三个球，藏起的不画。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { itemsOf, stepOf } from '../src/model/entries.js';

loadRes();

const AT = '2026-10-10T04:02:00.000Z';
const brief = (items) => items.map((it) => `${it.type}: ${it.type === 'steps' ? it.steps.map((s) => s.kind).join(',') : it.slot ? `[${it.slot}]` : it.text}`);

const turn3 = [
  { id: 'm2', kind: 'user', text: '你好', turn: 3, level: 'workspace', at: AT },
  { id: 'g6.0', kind: 'group', turn: 3, steps: ['b6.0', 'b6.1'], summary: [{ text: '思考了 1s' }], summary_en: [{ text: 'Thought for 1s' }, { text: ' · ' }, { text: '+3', tone: 'added' }], took_ms: 900, at: AT },
  { id: 'b6.0', kind: 'thought', group: 'g6.0', turn: 3, text: '想一想', took_ms: 300, at: AT },
  { id: 'b6.1', kind: 'tool', group: 'g6.0', turn: 3, call: 'c1', name: 'edit', state: 'ok', args: '{"file_path":"a.txt"}', title: { name: '编辑', object: '~/a.txt', said: '改了 1 处' }, diff: { added: 3, removed: 1 }, took_ms: 40, at: AT },
  { id: 'b6.2', kind: 'reply', turn: 3, text: '汪！', at: AT },
  { id: 'e9', kind: 'end', turn: 3, level: 'workspace', reason: 'completed', endpoint: 'dev', model: 'm1', took_ms: 2216, usage: { uncached: 3000, cache_read: 1000, cache_write: 0, output: 150 }, at: AT },
];

test('一轮：你的话（开轮那一句）、时间线的一段、她的回答、收尾那一行；收起那一行照核心给的（界面语言是自动的照英文那份）', () => {
  const got = itemsOf(turn3, { state: 'idle', permission: { level: 'workspace' } }, { account: 'admin' });
  assert.deepEqual(brief(got.items), ['user: 你好', 'steps: thought,tool', 'reply: 汪！', 'done: ▣  12:02 · dev/m1 · 2.2s · 4.2k(C25%)'.replace('12:02', got.items[3].text.match(/\d\d:\d\d/)[0])]);
  const user = got.items[0];
  assert.equal(user.opens, true);
  assert.deepEqual(user.speaker, { kind: 'person', account: 'admin', name: 'admin' });
  const seg = got.items[1];
  assert.equal(seg.finished, true);
  assert.deepEqual(seg.summary, { failed: false, spans: [{ text: 'Thought for 1s', tone: 'base' }, { text: ' · ', tone: 'base' }, { text: '+3', tone: 'added' }] });
  assert.equal(got.running, null);
  assert.equal(got.level, 'workspace');
});

test('一步：思考的起止照 took_ms；工具做完了照状态、带核心给的标题、改了多少行；在写参数的是 preparing', () => {
  const thought = stepOf(turn3[2]);
  assert.equal(thought.end - thought.start, 300);
  assert.equal(thought.state, 'done');
  const tool = stepOf(turn3[3]);
  assert.equal(tool.state, 'done');
  assert.equal(tool.status, 'ok');
  assert.equal(tool.duration, 40);
  assert.deepEqual(tool.title, { name: '编辑', object: '~/a.txt', said: '改了 1 处' });
  assert.deepEqual(tool.lines, { added: 3, removed: 1 });
  assert.deepEqual(tool.parsed, { file_path: 'a.txt' });
  const preparing = stepOf({ id: 'b7.0', kind: 'tool', name: 'shell', state: 'preparing', args: '{"comm', at: AT });
  assert.equal(preparing.state, 'preparing');
  assert.equal(preparing.parsed, null);
});

test('在跑：这一轮还没有她的条目画三个球；排着的话不进正文；撤回的、藏起的不画；在收的回答 streaming', () => {
  const entries = [
    ...turn3,
    { id: 'm10', kind: 'user', text: '再来', turn: 4, at: AT },
    { id: 'm11', kind: 'user', text: '排着的', turn: 4, queued: true, at: AT },
    { id: 'm12', kind: 'user', text: '撤回了', turn: 4, withdrawn: true, at: AT },
  ];
  const running = { state: 'running', since: AT, permission: { level: 'full' } };
  let got = itemsOf(entries, running);
  assert.deepEqual(brief(got.items).slice(-2), ['user: 再来', 'waiting: undefined']);
  assert.deepEqual(got.queued.map((q) => q.text), ['排着的']);
  assert.equal(got.running.turn, 4);
  assert.equal(got.running.start, Date.parse(AT));
  assert.equal(got.level, 'full');
  got = itemsOf([...entries, { id: 'b13.0', kind: 'reply', turn: 4, text: '写到一半', open: true, at: AT }], running);
  assert.deepEqual(brief(got.items).slice(-1), ['reply: 写到一半']);
  assert.equal(got.items.at(-1).streaming, true);
  got = itemsOf(turn3.map((e) => (e.turn === 3 ? { ...e, hidden: true } : e)), { state: 'idle' });
  assert.deepEqual(got.items, []);
});

test('收尾那一行：打断的接后台还在跑的任务数，出错的照出错那一句，别的原因照界面的字', () => {
  const end = (reason, extra = {}) => itemsOf([{ id: 'e1', kind: 'end', turn: 1, level: 'workspace', reason, at: AT, ...extra }], { state: 'idle', jobs: [{ job: 'j1', state: 'running' }, { job: 'j2', state: 'done' }] }).items[0];
  assert.equal(end('interrupted').interrupted, true);
  assert.equal(end('interrupted').jobs, 1);
  assert.equal(end('error', { error: { class: 'auth', status: 401, message: 'bad key' } }).tone, 'error');
  assert.match(end('error', { error: { class: 'auth', status: 401, message: 'bad key' } }).text, /bad key/);
  assert.match(end('step_limit').text, /已达请求次数上限/);
});

test('旁白：压缩、清空、后台任务、换了模型、工作区、别的会话、回顾照原来的写法；答题交给 asking 画；撤销那一条不画', () => {
  const notices = [
    { id: 'c20', kind: 'notice', what: 'compaction', trigger: 'auto', state: 'done', before: 120000, after: 30000, at: AT },
    { id: 'c21', kind: 'notice', what: 'compaction', trigger: 'auto', state: 'running', at: AT },
    { id: 'e22', kind: 'notice', what: 'cleared', at: AT },
    { id: 'e23', kind: 'notice', what: 'job', job: 'j1', job_kind: 'command', title: '跑测试', mark: 'done', reason: 'exited', command: 'npm test', output: { blob: 'sha256:x', chars: 10 }, at: AT },
    { id: 'e24', kind: 'notice', what: 'job', job: 'j2', job_kind: 'command', title: '编译', mark: 'failed', reason: 'exited', at: AT },
    { id: 'x24.0', kind: 'notice', what: 'model', why: 'failover', endpoint: 'dev', model: 'm2', at: AT },
    { id: 'e25', kind: 'notice', what: 'workspace', cwd: '/home/a/src', at: AT },
    { id: 'e26', kind: 'notice', what: 'peer', session: '0000abcd-1111', reason: 'expired', at: AT },
    { id: 'e27', kind: 'notice', what: 'answered', questions: [{ question: '选哪个？', options: [{ label: 'A' }] }], answers: [{ picked: ['A'] }], at: AT },
    { id: 'e28', kind: 'notice', what: 'reverted', turns: [3], said: '你好', at: AT },
    { id: 'e29', kind: 'notice', what: 'recap', text: '做了这些', covers: 3, at: AT },
  ];
  const got = itemsOf(notices, { state: 'idle' });
  assert.deepEqual(brief(got.items), [
    'note: 上下文已压缩：120k → 30k token',
    'note: 上下文已清空',
    'note: 后台命令完成 · 跑测试',
    'note: 后台命令失败 · 编译',
    'note: 已切换到 dev/m2（原模型出错）',
    'note: 工作区：/home/a/src',
    'note: 会话 bcd-1111 等待超时',
    'note: [asking]',
    'note: ',
  ]);
  assert.deepEqual(got.items[2].detail, { kind: 'output', command: 'npm test', hash: 'sha256:x', chars: 10 });
  assert.equal(got.items[1].compaction, 'clear');
  assert.equal(got.items[7].event.kind, 'question.answered');
  assert.equal(got.items[8].recap, '做了这些');
  assert.equal(got.items[8].covers, 3);
});

test('别处来的话写是谁：子代理照任务的标题，父会话写「父会话」', () => {
  const entries = [
    { id: 'm1', kind: 'user', text: '报告', from: { kind: 'session', id: 'kid-1' }, at: AT },
    { id: 'm2', kind: 'user', text: '交代', from: { kind: 'session', id: 'dad-1' }, at: AT },
  ];
  const got = itemsOf(entries, { state: 'idle', jobs: [{ job: 'j1', what: 'agent', title: '查文档', session: 'kid-1', state: 'running' }] }, { parent: 'dad-1' });
  assert.deepEqual(got.items.map((it) => it.speaker.name), ['子代理 · 查文档', '父会话']);
});

test('核心补上的格（9-8 三补）：后台命令的回报带退出码、用时；改文件的一步带真实路径，预览工作区照它', async () => {
  const { artifactEvents } = await import('../src/model/view-state.js');
  const jobs = [
    { id: 'e1', kind: 'notice', what: 'job', job: 'j1', job_kind: 'command', title: '跑测试', mark: 'done', took_ms: 20000, at: AT },
    { id: 'e2', kind: 'notice', what: 'job', job: 'j2', job_kind: 'command', title: '编译', mark: 'failed', exit_code: 101, at: AT },
    { id: 'e3', kind: 'notice', what: 'job', job: 'j3', job_kind: 'command', title: '睡', mark: 'failed', signal: 9, at: AT },
  ];
  assert.deepEqual(itemsOf(jobs, { state: 'idle' }).items.map((it) => it.text), ['后台命令完成 · 跑测试 · 20.0s', '后台命令失败 · 编译 · 退出码 101', '后台命令失败 · 睡 · 信号 9']);
  const tool = { id: 'b9.0', kind: 'tool', name: 'write', state: 'ok', args: '{"file_path":"x.md"}', files: [{ path: '/real/ws/out/x.md', action: 'changed' }, { path: '/real/ws/out/y.md', action: 'trashed' }], turn: 2, at: AT };
  assert.deepEqual(artifactEvents([tool], '/ws')[0].body.effects, [{ kind: 'file.changed', path: '/real/ws/out/x.md' }, { kind: 'file.trashed', path: '/real/ws/out/y.md' }]);
  assert.deepEqual(artifactEvents([{ ...tool, files: undefined }], '/ws')[0].body.effects, [{ kind: 'file.changed', path: '/ws/x.md' }], '没有 files 的照参数拼工作目录');
});
