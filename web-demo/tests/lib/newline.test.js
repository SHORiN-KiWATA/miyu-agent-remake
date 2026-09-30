// @ts-check
//! 换行的按键（蓝图 `web.md`「按键」）：`Shift+Enter`、`Ctrl+J`（照 TUI）；带了别的修饰键的、输入法在选字的不算。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isNewline } from '../../src/lib/newline.js';

const key = (k, mods = {}) => ({ key: k, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, isComposing: false, ...mods });

test('Shift+Enter、Ctrl+J 是换行；Enter、Ctrl+Shift+J、Alt、Cmd 的不是；输入法选字时不算', () => {
  assert.equal(isNewline(key('Enter', { shiftKey: true })), true);
  assert.equal(isNewline(key('j', { ctrlKey: true })), true);
  assert.equal(isNewline(key('J', { ctrlKey: true })), true, '开着大写锁');
  assert.equal(isNewline(key('Enter')), false);
  assert.equal(isNewline(key('j', { ctrlKey: true, shiftKey: true })), false);
  assert.equal(isNewline(key('j', { ctrlKey: true, altKey: true })), false);
  assert.equal(isNewline(key('j', { metaKey: true })), false);
  assert.equal(isNewline(key('j', { ctrlKey: true, isComposing: true })), false);
});
