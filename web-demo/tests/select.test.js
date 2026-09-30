// @ts-check
//! 左栏多选（蓝图 `web.md`「左栏」的「批量删除」）：点一项勾上、再点去掉；Shift 点把上一次点的和这一次之间的都勾上。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { toggle, range } from '../src/model/select.js';

test('点一项：没勾的勾上，勾了的去掉', () => {
  assert.deepEqual([...toggle(new Set(['a']), 'b')], ['a', 'b']);
  assert.deepEqual([...toggle(new Set(['a', 'b']), 'a')], ['b']);
});

test('Shift 点：上一次点的和这一次之间（含两头）都勾上，照会话表的先后；上一次点的不在表里的只勾这一项', () => {
  const order = ['a', 'b', 'c', 'd', 'e'];
  assert.deepEqual([...range(new Set(['a']), order, 'b', 'd')].sort(), ['a', 'b', 'c', 'd']);
  assert.deepEqual([...range(new Set(), order, 'd', 'b')].sort(), ['b', 'c', 'd']);
  assert.deepEqual([...range(new Set(), order, 'zz', 'c')], ['c']);
});
