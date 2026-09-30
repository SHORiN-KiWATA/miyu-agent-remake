// @ts-check
//! 职能（蓝图 `web/architecture.md`「职能」）：配置写了的赢；没写的只有一个能用的用它；不止一个又没写的报错，不偷偷用第一个；
//! 选出来的变了通知（内核照它把选中的当服务发出去，用它的包跟着重来）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Seams } from '../../src/kernel/seams.js';

const theme = (id, usable = true) => ({ id, available: () => usable });

test('只有一个能用的：用它；一个都没有：说没有', () => {
  const s = new Seams();
  assert.deepEqual(s.choose('theme'), { error: 'none' });
  s.provide('theme', theme('morning'), 'p1');
  s.provide('theme', theme('broken', false), 'p2');
  assert.equal(s.choose('theme').provider?.id, 'morning');
});

test('不止一个能用、配置没写：报「不止一个」，不偷偷用第一个；写了的赢', () => {
  const s = new Seams();
  s.provide('theme', theme('morning'), 'p1');
  s.provide('theme', theme('tokyonight'), 'p2');
  assert.deepEqual(s.choose('theme'), { error: 'ambiguous', choices: ['morning', 'tokyonight'] });
  s.prefer({ theme: 'tokyonight' });
  assert.equal(s.choose('theme').provider?.id, 'tokyonight');
});

test('配置写的那个不在、不能用：报出来，不换成别的', () => {
  const s = new Seams();
  s.provide('theme', theme('morning'), 'p1');
  s.prefer({ theme: 'sepia' });
  assert.deepEqual(s.choose('theme'), { error: 'missing', wanted: 'sepia' });
});

test('提供者拿掉、配置改了：选出来的变了就通知', () => {
  const s = new Seams();
  const seen = [];
  s.watch((name, choice) => seen.push(`${name}:${choice.provider?.id ?? choice.error}`));
  const undo = s.provide('theme', theme('morning'), 'p1');
  s.provide('theme', theme('tokyonight'), 'p2');
  s.prefer({ theme: 'tokyonight' });
  undo();
  assert.deepEqual(seen, ['theme:morning', 'theme:ambiguous', 'theme:tokyonight']);
});
