// @ts-check
//! 输入框下面那一行：左边级别和模型，右边速度 · 上下文 · 累计；放不下时右边先丢速度、再丢累计
//! （`tui.md`「框下面那一行」）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, sampleLog } from './support.js';
import { footer, fit } from '../src/model/footer.js';

loadRes();

test('一个会话的底栏：级别照最后一次改的，模型照最后一次请求，数照 TUI 的算法', () => {
  const f = footer(sampleLog(), { window: 1_000_000 });
  assert.deepEqual(f.left, { level: 'workspace', label: '▣ 工作区', model: 'deepseek-v4', endpoint: 'deepseek' });
  assert.deepEqual(f.right.map((p) => p.text), ['5 tok/s', '2.3k/1M(0.2%)', 'Σ12.4k(C81%)']);
});

test('只读开着写只读；还是 0 的格子不写', () => {
  const f = footer(sampleLog(52), {});
  assert.equal(f.left.label, '⏸ 只读');
  const fresh = footer(sampleLog(1), { window: 1_000_000 });
  assert.equal(fresh.left.model, null);
  assert.deepEqual(fresh.right, []);
});

test('不知道窗口多大的只写用了多少', () => {
  const f = footer(sampleLog(), {});
  assert.equal(f.right[1].text, '2.3k');
});

test('放不下时先丢速度、再丢累计，上下文留到最后', () => {
  const parts = footer(sampleLog(), { window: 1_000_000 }).right;
  const width = (ps) => ps.map((p) => p.text).join(' · ').length;
  const kept = (room) => fit(parts, room, width).map((p) => p.text);
  assert.deepEqual(kept(100), ['5 tok/s', '2.3k/1M(0.2%)', 'Σ12.4k(C81%)']);
  assert.deepEqual(kept(30), ['2.3k/1M(0.2%)', 'Σ12.4k(C81%)']);
  assert.deepEqual(kept(20), ['2.3k/1M(0.2%)']);
  assert.deepEqual(kept(5), []);
});

test('点一下换下一级：工作区 → 开放权限 → 只读 → 工作区', async () => {
  const { nextLevel, levelLabel } = await import('../src/model/footer.js');
  assert.deepEqual(['workspace', 'full', 'read_only'].map(nextLevel), ['full', 'read_only', 'workspace']);
  assert.equal(levelLabel('full'), '⏵⏵ 开放权限');
});

test('换级别发给核心的参数：只读只开只读开关，常用的那一级不动；工作区、开放权限写级别并关掉只读', async () => {
  const { levelParams } = await import('../src/model/footer.js');
  assert.deepEqual(levelParams('read_only'), { read_only: true });
  assert.deepEqual(levelParams('workspace'), { level: 'workspace', read_only: false });
  assert.deepEqual(levelParams('full'), { level: 'full', read_only: false });
});

test('核心切了级别（session.policy_changed）底栏跟着换：只读盖过常用的那一级', () => {
  const created = { seq: 1, kind: 'session.created', body: { permission: { level: 'workspace', read_only: false } } };
  const changed = (permission) => ({ seq: 2, kind: 'session.policy_changed', body: { permission } });
  assert.equal(footer([created, changed({ level: 'full', read_only: false })], {}).left.level, 'full');
  assert.equal(footer([created, changed({ level: 'full', read_only: true })], {}).left.level, 'read_only');
});
