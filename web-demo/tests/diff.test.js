// @ts-check
//! 编辑、写入点开的差异：照参数按行比（蓝图 `tui.md`「编辑、写入点开的差异」，TUI 演示的 `diff.rs`）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compare, fromArgs } from '../src/model/diff.js';

/** 差异写成一行行字：`行号 标记 内容`，略过的是 `⋯`。 */
const brief = (d) => d.lines.map((l) => (l.mark === 'gap' ? '⋯' : `${l.number ?? ''}${{ keep: ' ', removed: '-', added: '+' }[l.mark]}${l.text}`));

test('写入：全部是加上的，带行号', () => {
  const d = fromArgs({ file_path: '/x', content: 'a\nb\n' });
  assert.deepEqual(brief(d), ['1+a', '2+b']);
  assert.equal(d.added, 2);
  assert.equal(d.removed, 0);
});

test('编辑：每一处按行比，不带行号；两处之间一行略过', () => {
  const d = fromArgs({ file_path: '/x', edits: [
    { old_string: 'a\nb\n', new_string: 'a\nc\nd\n' },
    { old_string: 'x', new_string: 'y' },
  ] });
  assert.deepEqual(brief(d), [' a', '-b', '+c', '+d', '⋯', '-x', '+y']);
  assert.equal(d.added, 3);
  assert.equal(d.removed, 2);
});

test('改动前后只带三行不变的；隔得远的两处分开，中间略过', () => {
  const before = Array.from({ length: 20 }, (_, i) => `l${i + 1}`).join('\n') + '\n';
  const after = before.replace('l2\n', 'L2\n').replace('l18\n', 'L18\n');
  assert.deepEqual(brief(compare(before, after)), [
    '1 l1', '2-l2', '2+L2', '3 l3', '4 l4', '5 l5',
    '⋯',
    '15 l15', '16 l16', '17 l17', '18-l18', '18+L18', '19 l19', '20 l20',
  ]);
});

test('一样的没有行；最后一行有没有换行算不一样', () => {
  assert.deepEqual(brief(compare('a\n', 'a\n')), []);
  assert.deepEqual(brief(compare('a', 'a\n')), ['1-a', '1+a']);
});

test('既不是写入也不是编辑的参数没有差异', () => {
  assert.equal(fromArgs({ command: 'ls' }), null);
  assert.equal(fromArgs(null), null);
});
