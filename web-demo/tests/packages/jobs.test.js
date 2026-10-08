// @ts-check
//! 后台任务（软件包 `jobs`，蓝图 `web.md`「后台任务」）：哪些算在跑、结束的是什么状态、怎么排。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { tasksOf, compareJobs, running, childrenOf } from '../../src/lib/jobs.js';

/** 一条事件：`at` 是从 10:00:00 起的秒数。 */
const ev = (seq, at, kind, body, turn = 3) => ({ seq, at: new Date(Date.UTC(2026, 8, 30, 10, 0, 0) + at * 1000).toISOString(), kind, turn, body });
const started = (seq, at, job, what, title, session) => ev(seq, at, 'tool.result', { call_id: `c${seq}`, status: 'ok', blocks: [], effects: [{ kind: 'job.started', job, what, title, ...(session ? { session } : {}) }] });

test('派出去还没报的在跑；报过的照回报；子代理报过又被留言叫醒的又算在跑', () => {
  const events = [
    started(1, 0, 'j1', 'command', '跑测试'),
    started(2, 1, 'j2', 'agent', '查文档', 's-child'),
    started(3, 2, 'j3', 'command', '编译'),
    ev(4, 5, 'job.reported', { job: 'j1', reason: 'exited', exit_code: 0, duration_ms: 5000 }, undefined),
    ev(5, 6, 'child.reported', { job: 'j2', session: 's-child', reason: 'done', text: '好了' }, undefined),
    ev(6, 7, 'tool.result', { call_id: 'c6', status: 'ok', blocks: [], effects: [{ kind: 'job.messaged', job: 'j2' }] }),
    ev(7, 8, 'job.reported', { job: 'j3', reason: 'exited', exit_code: 2 }, undefined),
  ];
  const tasks = tasksOf(events);
  assert.deepEqual(tasks.map((x) => [x.job, x.what, x.title, x.state]), [
    ['j2', 'agent', '查文档', 'running'],
    ['j1', 'command', '跑测试', 'done'],
    ['j3', 'command', '编译', 'failed'],
  ]);
  assert.equal(tasks[0].since, Date.UTC(2026, 8, 30, 10, 0, 7), '被叫醒的从留言那一刻算');
  assert.equal(tasks[1].duration, 5000);
  assert.equal(tasks[2].code, 2);
  assert.equal(running(tasks), 1);
});

test('停掉、撤销、中断各是各的状态；信号杀掉的算失败', () => {
  const events = [
    started(1, 0, 'j1', 'command', 'a'),
    started(2, 0, 'j2', 'command', 'b'),
    started(3, 0, 'j3', 'agent', 'c', 's3'),
    started(4, 0, 'j4', 'command', 'd'),
    ev(5, 1, 'job.reported', { job: 'j1', reason: 'stopped', by_model: true }, undefined),
    ev(6, 1, 'job.reported', { job: 'j2', reason: 'exited', signal: 9 }, undefined),
    ev(7, 1, 'child.reported', { job: 'j3', session: 's3', reason: 'undone', text: '' }, undefined),
    ev(8, 1, 'job.reported', { job: 'j4', reason: 'aborted' }, undefined),
  ];
  assert.deepEqual(tasksOf(events).map((x) => [x.job, x.state]), [['j1', 'stopped'], ['j2', 'failed'], ['j3', 'undone'], ['j4', 'aborted']]);
  assert.equal(running(tasksOf(events)), 0);
});

test('编号一段一段按数比：j2 在 j2.1 前面，j2.9 在 j10 前面', () => {
  const ids = ['j10', 'j2.1', 'j2', 'j2.9', 'j1', 'j2.10'];
  assert.deepEqual([...ids].sort(compareJobs), ['j1', 'j2', 'j2.1', 'j2.9', 'j2.10', 'j10']);
});

test('会话底下挂的子代理：派出去的 agent 带子会话的，照编号排，最新的在前；在跑的记着', () => {
  const events = [
    started(1, 0, 'j1', 'agent', '查文档', 's1'),
    started(2, 1, 'j2', 'command', '编译'),
    started(3, 2, 'j3', 'agent', '数文件', 's3'),
    ev(4, 3, 'child.reported', { job: 'j1', session: 's1', reason: 'done', text: '' }, undefined),
  ];
  assert.deepEqual(childrenOf(events).map((c) => [c.session, c.title, c.running]), [['s3', '数文件', true], ['s1', '查文档', false]]);
});

test('停在半路：子代理的会话最后一轮是被打断的、之后没开新的一轮、也没回报——不算在跑，算 paused', () => {
  const parent = [started(1, 0, 'j1', 'agent', '查文档', 's1'), started(2, 0, 'j2', 'agent', '数文件', 's2'), started(3, 0, 'j3', 'agent', '写报告', 's3')];
  const child = {
    s1: [ev(1, 0, 'turn.started', { trigger: 1 }, 1), ev(2, 3, 'turn.ended', { reason: 'interrupted' }, 1)],
    s2: [ev(1, 0, 'turn.started', { trigger: 1 }, 1)],
    s3: [ev(1, 0, 'turn.started', { trigger: 1 }, 1), ev(2, 3, 'turn.ended', { reason: 'interrupted' }, 1), ev(3, 5, 'turn.started', { trigger: 3 }, 3)],
  };
  const tasks = tasksOf(parent, (id) => child[id] ?? null);
  assert.deepEqual(tasks.map((x) => [x.job, x.state]), [['j2', 'running'], ['j3', 'running'], ['j1', 'paused']]);
  assert.equal(running(tasks), 2, '停在半路的不算在跑');
  assert.deepEqual(childrenOf(parent, (id) => child[id] ?? null).map((c) => [c.session, c.running, c.paused]), [['s3', true, false], ['s2', true, false], ['s1', false, true]]);
});

test('后台命令带着命令本身（照派它的那次 shell 调用的参数，浮层的预览用）；子代理、读不懂的参数没有', () => {
  const called = (seq, call_id, args) => ev(seq, 0, 'message.assistant', { blocks: [{ type: 'tool_call', call_id, name: 'shell', args }] });
  const events = [
    called(1, 'c2', JSON.stringify({ command: 'cargo test --workspace', background: true })),
    started(2, 1, 'j1', 'command', '跑测试'),
    called(3, 'c4', '{not json'),
    started(4, 2, 'j2', 'command', '编译'),
    started(5, 3, 'j3', 'agent', '查文档', 's-child'),
  ];
  assert.deepEqual(tasksOf(events).map((x) => [x.job, x.command]), [['j1', 'cargo test --workspace'], ['j2', null], ['j3', null]]);
});

test('一个会话里在跑的后台任务一共几个：连子代理（读进来了的）再派的一起数；绕回来的不数两遍', async () => {
  const { runningDeep } = await import('../../src/lib/jobs.js');
  const parent = [started(1, 0, 'j1', 'command', '跑测试'), started(2, 1, 'j2', 'agent', '查文档', 's-child')];
  const child = { 's-child': [started(1, 0, 'j1', 'command', '子代理跑的'), started(2, 1, 'j2', 'agent', '绕回去', 's-parent')], 's-parent': parent };
  assert.equal(runningDeep('s-parent', (id) => child[id] ?? null), 4, 'j1、j2，子代理的 j1、j2；绕回父会话的不再数');
  assert.equal(runningDeep('s-none', () => null), 0);
});

test('还在跑的任务的种子（核心 9-6 上）：派它的那条还没读进来时照种子算在跑、不知道几时开始；读到真的那条照真的；之后的回报照样结束它', () => {
  const seed = { seq: 0, at: '2026-10-08T00:00:00Z', kind: 'jobs.seed', body: { jobs: [{ job: 'j1', what: 'command', title: '编译' }, { job: 'j2', what: 'agent', title: '查资料', session: 'S2' }] } };
  const tasks = tasksOf([seed]);
  assert.deepEqual(tasks.map((x) => [x.job, x.state, x.since, x.session]), [['j1', 'running', null, null], ['j2', 'running', null, 'S2']]);
  assert.deepEqual(childrenOf([seed]).map((x) => [x.session, x.running]), [['S2', true]], '左栏的子代理树照样有它');
  const reported = { seq: 30, at: '2026-10-08T00:01:00Z', kind: 'job.reported', body: { job: 'j1', reason: 'exited', exit_code: 0, duration_ms: 1200 } };
  assert.equal(tasksOf([seed, reported]).find((x) => x.job === 'j1')?.state, 'done');
});
