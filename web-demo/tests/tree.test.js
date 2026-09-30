// @ts-check
//! 左栏的树（蓝图 `web.md`「左栏」的「子代理的会话」「会话表」）：一层里在跑的、停在半路的一个一行，结束的收成「已完成 N 个」；
//! 孙代理默认收着，子代理带着下面挂了几个；父会话默认有在跑的、看着的在它下面的展开；文件树的线；顶层露几个。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { treeRows, capTops, pathOf } from '../src/model/tree.js';

const top = (id, extra = {}) => ({ session: id, title: id, running: false, unread: false, ...extra });
const kid = (id, running = false, paused = false) => ({ session: id, title: `子 ${id}`, running, paused });

/** 行写成一串：层数、编号、开关（▸▾）、下面挂几个（(N)）、收着的里面在跑（*）；「已完成 N 个」写成 done:N（开着的 done:N▾） */
const brief = (rows) => rows.map((r) => (r.kind === 'done' ? `${r.depth}:done:${r.count}${r.open ? '▾' : ''}` : `${r.depth}:${r.session}${r.hasKids ? (r.open ? '▾' : '▸') : ''}${r.kidCount ? `(${r.kidCount})` : ''}${r.busy ? '*' : ''}`));
const state = (extra = {}) => ({ current: null, open: new Map(), done: new Set(), ...extra });

test('一层里：在跑的、停在半路的一个一行，结束的收成「已完成 N 个」；点开了列出来', () => {
  const kids = new Map([['a', [kid('a1', true), kid('a2'), kid('a3', false, true), kid('a4')]]]);
  const of = (id) => kids.get(id) ?? [];
  assert.deepEqual(brief(treeRows([top('a')], of, state())), ['0:a▾', '1:a1', '1:a3', '1:done:2']);
  assert.deepEqual(brief(treeRows([top('a')], of, state({ done: new Set(['a']) }))), ['0:a▾', '1:a1', '1:a3', '1:done:2▾', '1:a2', '1:a4']);
});

test('父会话默认：有在跑的展开；看着父会话、都跑完了的收着；看着的在下面的展开', () => {
  const kids = new Map([['a', [kid('a1')]], ['b', [kid('b1', true)]]]);
  const of = (id) => kids.get(id) ?? [];
  assert.deepEqual(brief(treeRows([top('a'), top('b')], of, state({ current: 'a' }))), ['0:a▸', '0:b▾', '1:b1']);
  assert.deepEqual(brief(treeRows([top('a')], of, state({ current: 'a1' }))), ['0:a▾', '1:done:1▾', '1:a1'], '看着的在「已完成」里：那一段也展开');
});

test('孙代理默认收着：子代理带着下面挂了几个；看着孙代理的、人点开的展开；收着的里面在跑记着', () => {
  const kids = new Map([['a', [kid('a1', true)]], ['a1', [kid('x1', true), kid('x2')]]]);
  const of = (id) => kids.get(id) ?? [];
  assert.deepEqual(brief(treeRows([top('a')], of, state())), ['0:a▾', '1:a1▸(1)*']);
  assert.deepEqual(brief(treeRows([top('a')], of, state({ current: 'x2' }))), ['0:a▾', '1:a1▾(1)', '2:x1', '2:done:1▾', '2:x2']);
  assert.deepEqual(brief(treeRows([top('a')], of, state({ open: new Map([['a1', true]]) }))), ['0:a▾', '1:a1▾(1)', '2:x1', '2:done:1']);
});

test('文件树的线：每一行知道自己是不是这一层最后一个、上面几层要不要往下画竖线', () => {
  const kids = new Map([['a', [kid('a1', true), kid('a2', true)]], ['a1', [kid('x', true)]], ['a2', [kid('y', true)]]]);
  const rows = treeRows([top('a')], (id) => kids.get(id) ?? [], state({ open: new Map([['a1', true], ['a2', true]]) }));
  assert.deepEqual(rows.map((r) => `${r.kind === 'session' ? r.session : r.kind}:${r.last ? 'last' : 'mid'}:${(r.guides ?? []).map((g) => (g ? '|' : ' ')).join('')}`),
    ['a:last:', 'a1:mid:', 'x:last:|', 'a2:last:', 'y:last: ']);
});

test('顶层最多露几个；正在看的不在里面的排在最后照样露', () => {
  const tops = ['a', 'b', 'c', 'd'].map((id) => top(id));
  assert.deepEqual(capTops(tops, 'b', 2).shown.map((x) => x.session), ['a', 'b']);
  assert.deepEqual(capTops(tops, 'd', 2).shown.map((x) => x.session), ['a', 'b', 'd']);
  assert.equal(capTops(tops, 'd', 2).more, true);
  assert.equal(capTops(tops.slice(0, 2), null, 2).more, false);
});

test('路径：从主会话一层层到这个会话；绕回来的不走两遍', () => {
  const parent = new Map([['x', 'a1'], ['a1', 'a']]);
  assert.deepEqual(pathOf('x', (id) => parent.get(id) ?? null), ['a', 'a1', 'x']);
  assert.deepEqual(pathOf('a', (id) => parent.get(id) ?? null), ['a']);
  const loop = new Map([['p', 'q'], ['q', 'p']]);
  assert.deepEqual(pathOf('p', (id) => loop.get(id) ?? null), ['q', 'p']);
});

test('(N) 只算在跑的子代理：顶层收着的也写；结束的、停在半路的不算，一个都没在跑的不写；展开了不写', () => {
  const kids = new Map([['a', [kid('a1', true), kid('a2', true), kid('a3'), kid('a4', false, true)]], ['b', [kid('b1')]]]);
  const of = (id) => kids.get(id) ?? [];
  const closed = state({ open: new Map([['a', false]]) });
  assert.deepEqual(brief(treeRows([top('a'), top('b')], of, closed)), ['0:a▸(2)*', '0:b▸']);
  assert.deepEqual(brief(treeRows([top('a')], of, state())), ['0:a▾', '1:a1', '1:a2', '1:a4', '1:done:1']);
});

test('(N) 连子代理再派的一起数：一层层往下在跑的都算，结束的、停在半路的不算；绕回来的不数两遍', () => {
  const kids = new Map([
    ['a', [kid('a1', true), kid('a2')]],
    ['a1', [kid('x1', true), kid('x2', true), kid('x3'), kid('x4', false, true)]],
    ['a2', [kid('y1', true)]],
    ['x1', [kid('a1', true)]],
  ]);
  const of = (id) => kids.get(id) ?? [];
  const closed = state({ open: new Map([['a', false]]) });
  assert.deepEqual(brief(treeRows([top('a')], of, closed)), ['0:a▸(4)*'], 'a1、x1、x2，结束了的 a2 下面在跑的 y1 也算');
  assert.deepEqual(brief(treeRows([top('a')], of, state())).slice(0, 2), ['0:a▾', '1:a1▸(2)*'], 'a1 下面：x1、x2（x1 绕回 a1 的不再数）');
});
