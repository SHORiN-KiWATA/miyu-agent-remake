// @ts-check
//! 后台任务（软件包 `jobs`，蓝图 `web.md`「后台任务」）：照会话状态的任务表（核心 9-8）认哪些算在跑、结束的是什么状态、怎么排；
//! 左栏挂的子代理；连子孙一共几个在跑。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { tasksFromStatus, childrenFromStatus, runningDeepStatus, compareJobs, running } from '../../src/lib/jobs.js';

const at = (s) => new Date(Date.UTC(2026, 8, 30, 10, 0, 0) + s * 1000).toISOString();

test('任务表：在跑的排前面、结束的排后面，各照编号从新到旧；用时照起止、退出码、信号、命令照原样', () => {
  const tasks = tasksFromStatus([
    { job: 'j1', what: 'command', title: '跑测试', state: 'done', started: at(0), ended: at(5), exit_code: 0, command: 'cargo test' },
    { job: 'j2', what: 'agent', title: '查文档', state: 'running', started: at(1), session: 's-child' },
    { job: 'j3', what: 'command', title: '编译', state: 'failed', started: at(2), ended: at(8), exit_code: 2 },
    { job: 'j4', what: 'command', title: '睡', state: 'failed', started: at(2), ended: at(3), signal: 9 },
  ]);
  assert.deepEqual(tasks.map((x) => [x.job, x.what, x.title, x.state]), [
    ['j2', 'agent', '查文档', 'running'],
    ['j4', 'command', '睡', 'failed'],
    ['j3', 'command', '编译', 'failed'],
    ['j1', 'command', '跑测试', 'done'],
  ]);
  assert.deepEqual([tasks[0].since, tasks[0].ended, tasks[0].duration, tasks[0].session], [Date.parse(at(1)), null, null, 's-child']);
  assert.deepEqual([tasks[3].duration, tasks[3].code, tasks[3].command], [5000, 0, 'cargo test']);
  assert.deepEqual([tasks[2].code, tasks[1].signal], [2, 9]);
  assert.equal(running(tasks), 1);
  assert.deepEqual(tasksFromStatus(null), []);
});

test('停掉的照 why 写（人停、随撤销、因重启），核心崩了断了的是 aborted', () => {
  const tasks = tasksFromStatus([
    { job: 'j1', what: 'command', title: 'a', state: 'stopped', why: 'stopped', started: at(0) },
    { job: 'j2', what: 'agent', title: 'b', state: 'stopped', why: 'undone', started: at(0), session: 's2' },
    { job: 'j3', what: 'command', title: 'c', state: 'stopped', why: 'restarted', started: at(0) },
    { job: 'j4', what: 'command', title: 'd', state: 'aborted', started: at(0) },
  ]);
  assert.deepEqual(tasks.map((x) => [x.job, x.state]), [['j4', 'aborted'], ['j3', 'restarted'], ['j2', 'undone'], ['j1', 'stopped']]);
  assert.equal(running(tasks), 0);
});

test('编号一段一段按数比：j2 在 j2.1 前面，j2.9 在 j10 前面', () => {
  const ids = ['j10', 'j2.1', 'j2', 'j2.9', 'j1', 'j2.10'];
  assert.deepEqual([...ids].sort(compareJobs), ['j1', 'j2', 'j2.1', 'j2.9', 'j2.10', 'j10']);
});

test('会话底下挂的子代理：派出去的 agent 带子会话的，照编号排，最新的在前；在跑的记着；后台命令不算', () => {
  const kids = childrenFromStatus([
    { job: 'j1', what: 'agent', title: '查文档', state: 'done', started: at(0), session: 's1' },
    { job: 'j2', what: 'command', title: '编译', state: 'running', started: at(1) },
    { job: 'j3', what: 'agent', title: '数文件', state: 'running', started: at(2), session: 's3' },
  ]);
  assert.deepEqual(kids.map((c) => [c.session, c.job, c.title, c.running]), [['s3', 'j3', '数文件', true], ['s1', 'j1', '查文档', false]]);
});

test('一个会话里在跑的后台任务一共几个：核心算好了整棵树的照它；没有的连读进来了的子代理再派的一起数，绕回来的不数两遍', () => {
  const status = {
    's-parent': { jobs: [{ job: 'j1', what: 'command', state: 'running' }, { job: 'j2', what: 'agent', state: 'running', session: 's-child' }] },
    's-child': { jobs: [{ job: 'j1', what: 'command', state: 'running' }, { job: 'j2', what: 'agent', state: 'running', session: 's-parent' }] },
  };
  assert.equal(runningDeepStatus('s-parent', (id) => status[id] ?? null), 4, 'j1、j2，子代理的 j1、j2；绕回父会话的不再数');
  assert.equal(runningDeepStatus('s-parent', () => ({ running_deep: 7, jobs: [] })), 7, '核心给了 running_deep 的照它');
  assert.equal(runningDeepStatus('s-none', () => null), 0);
});
