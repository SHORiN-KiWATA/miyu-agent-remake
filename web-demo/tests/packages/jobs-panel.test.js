// @ts-check
//! 后台任务浮层怎么分段（蓝图 `web.md`「后台任务」第 3 条）：「进行中」列在跑的、停在半路的，子代理下面在跑的接在它下面；
//! 「已结束」列结束的；嵌套里结束的不单列，算进它那一层的「派了 N 个」。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { sections } from '../../packages/jobs/sections.js';

const node = (job, what, state, kids = []) => ({ job, what, title: job, state, owner: 's', session: what === 'agent' ? `s-${job}` : null, since: 0, ended: null, duration: null, code: null, signal: null, kids });

test('进行中、已结束两段；嵌套的在跑的接在下面，结束的只算数', () => {
  const tree = [
    node('j1', 'agent', 'running', [node('j1.1', 'agent', 'running'), node('j1.2', 'agent', 'done'), node('j1.3', 'command', 'failed')]),
    node('j2', 'command', 'done'),
    node('j3', 'agent', 'paused'),
    node('j4', 'agent', 'stopped', [node('j4.1', 'agent', 'running')]),
  ];
  const s = sections(tree);
  assert.deepEqual(s.live.map((r) => `${r.depth}:${r.task.job}:${r.spawned}`), ['0:j1:3', '1:j1.1:0', '0:j3:0', '1:j4.1:0']);
  assert.deepEqual(s.done.map((r) => `${r.task.job}:${r.spawned}`), ['j2:0', 'j4:1']);
  assert.deepEqual(s.counts, { live: 4, done: 2 });
});

test('进行中的树线：同一层最后一个、上面几层的竖线、下面接没接；父子代理结束了的不画线', () => {
  const tree = [
    node('j1', 'agent', 'running', [
      node('j1.1', 'agent', 'running', [node('j1.1.1', 'agent', 'running')]),
      node('j1.2', 'agent', 'paused'),
    ]),
    node('j4', 'agent', 'stopped', [node('j4.1', 'agent', 'running')]),
  ];
  const lines = sections(tree).live.map((r) => `${r.task.job}:${r.last ? 'last' : 'mid'}:${r.guides.map((g) => (g ? '|' : ' ')).join('')}:${r.kids ? 'K' : ''}${r.orphan ? 'O' : ''}`);
  assert.deepEqual(lines, ['j1:last::K', 'j1.1:mid::K', 'j1.1.1:last:|:', 'j1.2:last::', 'j4.1:last::O']);
});
