// @ts-check
//! `@` 选的文件、目录在框里是一块（蓝图 `web.md`「`@` 选文件」第 5 条，照 `tui.md` 的文件块）：块怎么起名、整块删、发出去换回路径、
//! 底下垫的那层字怎么切。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { blockLabel, expand, used, erase, pieces } from '../src/model/blocks.js';

test('起名：[文件名]，目录 [名字/]；太长的中间截掉留扩展名；同名不同路径的写上一层目录', () => {
  const none = new Map();
  assert.equal(blockLabel('src/ui/history.js', false, none, 24), '[history.js]');
  assert.equal(blockLabel('src/ui/', true, none, 24), '[ui/]');
  assert.equal(blockLabel('docs/a-very-long-file-name-for-testing.md', false, none, 24), '[a-very-long-fil…sting.md]');
  const taken = new Map([['[history.js]', 'src/ui/history.js']]);
  assert.equal(blockLabel('src/model/history.js', false, taken, 24), '[model/history.js]');
  assert.equal(blockLabel('src/ui/history.js', false, taken, 24), '[history.js]', '同一个路径还是同一个名字');
});

test('发出去：块换回路径写在原来的位置；没登记的方括号照原样', () => {
  const blocks = new Map([['[a.rs]', 'src/a.rs'], ['[docs/]', "'my docs/'"]]);
  assert.equal(expand('看看 [a.rs] 和 [docs/] 还有 [别的]', blocks), "看看 src/a.rs 和 'my docs/' 还有 [别的]");
  assert.deepEqual(used('只有 [a.rs]', blocks), [['[a.rs]', 'src/a.rs']]);
});

test('整块删：光标在块后面退格、在块前面 Delete 删整块；别处交 null', () => {
  const labels = ['[a.rs]'];
  assert.deepEqual(erase('看 [a.rs] 吧', 8, labels, 'back'), { value: '看  吧', caret: 2 });
  assert.deepEqual(erase('看 [a.rs] 吧', 2, labels, 'forward'), { value: '看  吧', caret: 2 });
  assert.equal(erase('看 [a.rs] 吧', 9, labels, 'back'), null);
  assert.equal(erase('看 [a.rs] 吧', 5, labels, 'back'), null, '在块中间');
});

test('底下垫的那层字：块单独成一段，别的照原样', () => {
  assert.deepEqual(pieces('看 [a.rs] 和 [a.rs]', ['[a.rs]']), [
    { text: '看 ', block: false }, { text: '[a.rs]', block: true }, { text: ' 和 ', block: false }, { text: '[a.rs]', block: true },
  ]);
  assert.deepEqual(pieces('没有块', ['[a.rs]']), [{ text: '没有块', block: false }]);
});
