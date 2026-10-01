// @ts-check
//! 时间线的段和步（蓝图 `web.md`「时间线的数」，规矩照 `tui.md`「时间线」）：段怎么分、说话就收、结果对回那一步、
//! 一轮结束时没结果的记成打断、在收的块、同一时刻只转一处、三个球什么时候有。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, sampleLog, ev, ms } from './support.js';
import { project } from '../src/model/transcript.js';
import { active } from '../src/model/timeline.js';

loadRes();

const MODEL = { kind: 'model', endpoint: 'deepseek', model: 'deepseek-v4' };

/** 一轮：先想、跑一条命令、改一个文件（出错），再想一下开口。 */
function worked() {
  return [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '整理一下' }] }),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 5, 'message.assistant', 3, { seen: 3, blocks: [
      { type: 'reasoning', text: '先看看目录\n再决定' },
      { type: 'tool_call', call_id: 'c1', name: 'shell', args: '{"command":"ls -la src","description":"列目录"}' },
      { type: 'tool_call', call_id: 'c2', name: 'edit', args: '{"file_path":"/home/me/a.rs","edits":[{"old_string":"a\\nb\\n","new_string":"a\\nc\\nd\\n"}]}' },
    ] }, MODEL),
    ev(5, 6, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'text', text: 'a.rs\nb.rs' }], duration_ms: 800 }),
    ev(6, 7, 'tool.result', 3, { call_id: 'c2', status: 'error', blocks: [{ type: 'text', text: 'no such file' }], human: { key: 'software/basesystem/common/missing', fields: {} } }),
    ev(7, 9, 'message.assistant', 3, { seen: 6, blocks: [{ type: 'reasoning', text: '好了' }, { type: 'text', text: '整理完了。' }] }, MODEL),
    ev(8, 10, 'turn.ended', 3, { reason: 'completed' }),
  ];
}

test('思考、调工具记进一段，她开口才收：两次请求之间没开口的是同一段', () => {
  const items = project(worked()).items;
  assert.deepEqual(items.map((it) => it.type), ['user', 'steps', 'reply', 'done']);
  const seg = items[1];
  assert.equal(seg.key, 't3-1');
  assert.equal(seg.finished, true);
  assert.deepEqual(seg.steps.map((s) => `${s.kind}:${s.name ?? ''}:${s.state}:${s.status ?? ''}`),
    ['thought::done:', 'tool:shell:done:ok', 'tool:edit:done:error', 'thought::done:']);
});

test('结果照调用编号对回那一步：输出、结果那一句、用时从回复落盘到结果', () => {
  const [, shell, edit] = project(worked()).items[1].steps;
  assert.equal(shell.output, 'a.rs\nb.rs');
  assert.deepEqual(shell.parsed, { command: 'ls -la src', description: '列目录' });
  assert.deepEqual([shell.start, shell.end], [ms(5), ms(6)]);
  assert.deepEqual(edit.said, { key: 'software/basesystem/common/missing', fields: {} });
  assert.deepEqual([edit.start, edit.end], [ms(5), ms(7)]);
});

test('读回来的历史里思考不知道想了多久', () => {
  const thought = project(worked()).items[1].steps[0];
  assert.equal(thought.text, '先看看目录\n再决定');
  assert.deepEqual([thought.start, thought.end], [null, null]);
});

test('读回来的历史：思考照 model.called 的 blocks 算（请求发出去的时刻是 at 减 duration_ms），照 seen 对上那条回复', () => {
  const events = worked();
  // 第 4 条回复（seen 3）三块：思考 0.5–2.5 秒、两次调工具；请求 5 秒时收完、用了 4 秒，发出去在 1 秒
  events.splice(4, 0, ev(4.5, 5, 'model.called', 3, { seen: 3, duration_ms: 4000, first_token_ms: 500,
    blocks: [{ start_ms: 500, end_ms: 2500 }, { start_ms: 2600, end_ms: 3000 }, { start_ms: 3100, end_ms: 3900 }] }, MODEL));
  const steps = project(events).items[1].steps;
  assert.deepEqual([steps[0].start, steps[0].end], [ms(1.5), ms(3.5)]);
  assert.deepEqual([steps[3].start, steps[3].end], [null, null], '没有 blocks 的那次请求照旧不知道');
});

test('看着流出来时记下的时刻优先于 blocks', () => {
  const events = worked();
  events.splice(4, 0, ev(4.5, 5, 'model.called', 3, { seen: 3, duration_ms: 4000, blocks: [{ start_ms: 500, end_ms: 2500 }, { start_ms: 2600, end_ms: 3000 }, { start_ms: 3100, end_ms: 3900 }] }, MODEL));
  const marks = new Map([['3:reasoning:0', { start: ms(1), end: ms(3.2) }]]);
  const [thought] = project(events, null, marks).items[1].steps;
  assert.deepEqual([thought.start, thought.end], [ms(1), ms(3.2)]);
});

test('页面看着流出来的思考，照记下的时刻；对上的是这次请求里第几块思考', () => {
  const marks = new Map([['3:reasoning:0', { start: ms(1), end: ms(3.5) }], ['6:reasoning:0', { start: ms(8), end: ms(8.4) }]]);
  const steps = project(worked(), null, marks).items[1].steps;
  assert.deepEqual([steps[0].start, steps[0].end], [ms(1), ms(3.5)]);
  assert.deepEqual([steps[3].start, steps[3].end], [ms(8), ms(8.4)]);
});

test('样本：她先开口再调工具，工具另起一段，下一句话收起它', () => {
  const items = project(sampleLog()).items.filter((it) => it.turn === 65);
  assert.deepEqual(items.map((it) => it.type), ['user', 'reply', 'steps', 'reply', 'done']);
  const [step] = items[2].steps;
  assert.equal(step.name, 'shell');
  assert.equal(step.status, 'denied');
  assert.equal(items[2].finished, true);
});

test('一轮结束了还没结果的步骤记成打断，停表', () => {
  const events = worked().slice(0, 4);
  events.push(ev(9, 12, 'turn.ended', 3, { reason: 'interrupted' }));
  const seg = project(events).items.find((it) => it.type === 'steps');
  assert.equal(seg.finished, true);
  assert.deepEqual(seg.steps.slice(1).map((s) => [s.state, s.status, s.end]), [['done', 'cancelled', ms(12)], ['done', 'cancelled', ms(12)]]);
});

test('在跑的：参数写完了等结果的，转最前面那个没结果的，后面的排队', () => {
  const seg = project(worked().slice(0, 4)).items.find((it) => it.type === 'steps');
  assert.equal(seg.finished, false);
  assert.deepEqual(seg.steps.slice(1).map((s) => s.state), ['running', 'running']);
  assert.equal(active(seg), 1);
});

test('在收的块：思考接字、调工具写参数时是准备；她还在写的转最后那一步', () => {
  const events = worked().slice(0, 3);
  const live = { turn: 3, seen: 3, blocks: [
    { kind: 'reasoning', text: '想一想', done: true, start: ms(1), end: ms(2) },
    { kind: 'tool_call', name: 'shell', text: '{"command":"l', done: false, start: ms(2), end: null },
  ] };
  const seg = project(events, live).items.at(-1);
  assert.equal(seg.type, 'steps');
  assert.equal(seg.key, 't3-1');
  assert.deepEqual(seg.steps.map((s) => [s.kind, s.state]), [['thought', 'done'], ['tool', 'preparing']]);
  assert.deepEqual([seg.steps[0].start, seg.steps[0].end], [ms(1), ms(2)]);
  assert.equal(active(seg), 1);
});

test('在收的回答收起前面那一段', () => {
  const live = { turn: 3, seen: 6, blocks: [{ kind: 'text', text: '整理', done: false, start: ms(9), end: null }] };
  const items = project(worked().slice(0, 6), live).items;
  assert.deepEqual(items.map((it) => it.type), ['user', 'steps', 'reply']);
  assert.equal(items[1].finished, true);
});

test('三个球：一轮开始、她还一块都没来时有；来了一块就没有；一轮结束了也没有', () => {
  const started = worked().slice(0, 3);
  assert.deepEqual(project(started).items.map((it) => it.type), ['user', 'waiting']);
  assert.equal(project(started).items[1].turn, 3);
  const live = { turn: 3, seen: 3, blocks: [{ kind: 'reasoning', text: '', done: false, start: ms(1), end: null }] };
  assert.ok(!project(started, live).items.some((it) => it.type === 'waiting'));
  const ended = [...started, ev(4, 3, 'turn.ended', 3, { reason: 'interrupted' })];
  assert.ok(!project(ended).items.some((it) => it.type === 'waiting'));
});

test('出过字以后，步与步之间等她的时候没有三个球', () => {
  const items = project(worked().slice(0, 6)).items;
  assert.ok(!items.some((it) => it.type === 'waiting'));
});


test('留言认出发给的是哪个子代理：照派它的那一步的标题（编号照结果里的 job.started），前面几轮派的也认得', () => {
  const items = project([
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '派一个去查' }] }),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 1, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'tool_call', call_id: 'c1', name: 'subagent', args: '{"description":"查文档","prompt":"去查"}' }] }, MODEL),
    ev(5, 2, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'text', text: 'started j2' }], effects: [{ kind: 'job.started', job: 'j2' }] }),
    ev(6, 3, 'message.assistant', 3, { seen: 5, blocks: [{ type: 'text', text: '派出去了。' }] }, MODEL),
    ev(7, 4, 'turn.ended', 3, { reason: 'completed' }),
    ev(8, 5, 'message.user', undefined, { blocks: [{ type: 'text', text: '告诉它先别改' }] }),
    ev(9, 5, 'turn.started', 9, { trigger: 8 }),
    ev(10, 6, 'message.assistant', 9, { seen: 9, blocks: [
      { type: 'tool_call', call_id: 'c2', name: 'send_message', args: '{"to":"j2","message":"先别改"}' },
      { type: 'tool_call', call_id: 'c3', name: 'send_message', args: '{"to":"j9","message":"x"}' },
    ] }, MODEL),
    ev(11, 7, 'tool.result', 9, { call_id: 'c2', status: 'ok', blocks: [{ type: 'text', text: 'Message sent to j2.' }] }),
    ev(12, 7, 'tool.result', 9, { call_id: 'c3', status: 'error', blocks: [{ type: 'text', text: 'not yours' }] }),
  ]).items;
  const steps = items.filter((it) => it.type === 'steps').flatMap((it) => it.steps).filter((s) => s.name === 'send_message');
  assert.deepEqual(steps.map((s) => s.toTitle ?? null), ['查文档', null]);
});

test('工具结果里的图记在那一步上：blob、类型、宽高（点开时画，蓝图「图片」第 2 条）', () => {
  const blob = `sha256:${'b'.repeat(64)}`;
  const log = [
    ev(1, 0, 'session.created', undefined, { permission: { level: 'workspace', read_only: false } }),
    ev(2, 0, 'message.user', undefined, { blocks: [{ type: 'text', text: '看图' }] }),
    ev(3, 0, 'turn.started', 3, { trigger: 2 }),
    ev(4, 1, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'tool_call', call_id: 'c1', name: 'read', args: '{"file_path":"/tmp/a.png"}' }] }),
    ev(5, 2, 'tool.result', 3, { call_id: 'c1', status: 'ok', blocks: [{ type: 'image', blob, media_type: 'image/png', width: 960, height: 540 }] }),
  ];
  const step = project(log).items.find((it) => it.type === 'steps').steps[0];
  assert.deepEqual(step.images, [{ blob, media_type: 'image/png', width: 960, height: 540 }]);
});
