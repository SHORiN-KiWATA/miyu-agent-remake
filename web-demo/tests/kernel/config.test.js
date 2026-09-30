// @ts-check
//! 配置（蓝图 `web/architecture.md`「配置」）：设置项的校验；出厂、发行版、个人三层合出最终值和来源；个人那一层写错的
//! 那一项退回下一层；清单自己的出厂值不对算这个包的毛病。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { check, resolve, problems } from '../../src/kernel/config.js';

const SETTINGS = {
  rows: { type: 'number', default: 10, min: 1, max: 40, integer: true },
  fold: { type: 'boolean', default: true },
  mode: { type: 'choice', default: 'auto', choices: ['auto', 'always', 'never'] },
  label: { type: 'text', default: '思考中' },
  color: { type: 'color', default: '#3a63c9' },
  icons: { type: 'map', of: 'text', default: { shell: 'terminal' } },
  order: { type: 'list', of: 'text', default: ['todo', 'pulse'] },
  sweep: { type: 'duration', default: 3000 },
};

test('每种设置项的校验：对的收下，不对的说为什么', () => {
  assert.deepEqual(check(SETTINGS.rows, 12), { ok: true, value: 12 });
  assert.equal(check(SETTINGS.rows, 0).ok, false);
  assert.equal(check(SETTINGS.rows, 2.5).ok, false);
  assert.equal(check(SETTINGS.rows, '3').ok, false);
  assert.equal(check(SETTINGS.fold, 'yes').ok, false);
  assert.equal(check(SETTINGS.mode, 'sometimes').ok, false);
  assert.equal(check(SETTINGS.mode, 'never').ok, true);
  assert.equal(check(SETTINGS.label, 3).ok, false);
  assert.equal(check(SETTINGS.color, '').ok, false);
  assert.equal(check(SETTINGS.icons, { read: 'file-text' }).ok, true);
  assert.equal(check(SETTINGS.icons, { read: 3 }).ok, false);
  assert.equal(check(SETTINGS.order, ['pulse']).ok, true);
  assert.equal(check(SETTINGS.order, 'pulse').ok, false);
  assert.equal(check(SETTINGS.sweep, -1).ok, false);
  assert.equal(check({ type: 'json', default: {} }, { tiers: [{ after: 0, words: ['想'] }] }).ok, true, 'json：一整块数据（词库这类）');
  assert.equal(check({ type: 'json', default: {} }, undefined).ok, false);
  const bad = check(SETTINGS.rows, 99);
  assert.ok(!bad.ok && /40/.test(bad.error), '写明范围');
});

test('三层合出最终值：个人盖发行版，发行版盖出厂；说得出每个值来自哪一层', () => {
  const r = resolve(SETTINGS, [{ name: 'distro', values: { rows: 8, fold: false } }, { name: 'user', values: { rows: 15 } }]);
  assert.equal(r.values.rows, 15);
  assert.equal(r.values.fold, false);
  assert.equal(r.values.mode, 'auto');
  assert.deepEqual([r.origins.rows, r.origins.fold, r.origins.mode], ['user', 'distro', 'default']);
  assert.deepEqual(r.errors, []);
});

test('个人那一层写错的那一项退回下一层，写明为什么；别的项照常', () => {
  const r = resolve(SETTINGS, [{ name: 'distro', values: { rows: 8 } }, { name: 'user', values: { rows: 999, mode: 'never' } }]);
  assert.equal(r.values.rows, 8);
  assert.equal(r.origins.rows, 'distro');
  assert.equal(r.values.mode, 'never');
  assert.equal(r.errors.length, 1);
  assert.equal(r.errors[0].key, 'rows');
  assert.equal(r.errors[0].layer, 'user');
});

test('不认识的项：警告、不算错，原样留着不用', () => {
  const r = resolve(SETTINGS, [{ name: 'user', values: { rowz: 3 } }]);
  assert.deepEqual(r.errors, []);
  assert.deepEqual(r.unknown, [{ key: 'rowz', layer: 'user' }]);
  assert.equal(r.values.rowz, undefined);
});

test('改回出厂：个人那一层没有这一项，就是下面的值', () => {
  const r = resolve(SETTINGS, [{ name: 'user', values: {} }]);
  assert.equal(r.values.rows, 10);
  assert.equal(r.origins.rows, 'default');
});

test('清单自己的毛病：出厂值过不了自己的校验、不认识的类型', () => {
  assert.deepEqual(problems(SETTINGS), []);
  const bad = problems({ a: { type: 'number', default: 'x' }, b: { type: 'weird', default: 1 } });
  assert.equal(bad.length, 2);
});
