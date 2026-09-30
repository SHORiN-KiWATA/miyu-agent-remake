// @ts-check
//! `@` 选文件（蓝图 `web.md`「`@` 选文件」）：光标前面的 `@` 词、两种找法、选定以后写进去的路径、`Tab` 进目录。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { wordAt, plan, pathText, dirWord, splice, failure } from '../src/model/mention.js';
import { loadRes } from './support.js';

loadRes();

test('光标前面是 @ 打头的词才开：@ 在最前面或者前面是空白；a@b.com 不算；到光标为止没有空白（\\ 算词里的空格）', () => {
  assert.deepEqual(wordAt('看看 @src/ma', 10), { start: 3, end: 10, word: 'src/ma' });
  assert.deepEqual(wordAt('@', 1), { start: 0, end: 1, word: '' });
  assert.equal(wordAt('写信给 a@b.com', 12), null);
  assert.equal(wordAt('@src 然后', 7), null, '光标前面有空白');
  assert.deepEqual(wordAt('@my\\ docs/a', 11), { start: 0, end: 11, word: 'my docs/a' });
  assert.equal(wordAt('没有', 2), null);
});

test('两种找法：带 / 或者 ~、/ 打头的按目录找（只读那一层）；别的在工作目录里模糊找；只打了 @ 列工作目录这一层', () => {
  assert.deepEqual(plan('src/ma'), { mode: 'dir', dir: 'src/', prefix: 'ma' });
  assert.deepEqual(plan('~/Doc'), { mode: 'dir', dir: '~/', prefix: 'Doc' });
  assert.deepEqual(plan('~'), { mode: 'dir', dir: '~', prefix: '' });
  assert.deepEqual(plan('/etc/'), { mode: 'dir', dir: '/etc/', prefix: '' });
  assert.deepEqual(plan(''), { mode: 'dir', dir: '', prefix: '' });
  assert.deepEqual(plan('main'), { mode: 'find', query: 'main' });
});

test('写进话里的路径：在工作目录里的写相对路径，家目录里的写 ~/…，别处写绝对路径；带空白、引号的加单引号', () => {
  const cwd = '/home/me/proj';
  const home = '/home/me';
  assert.equal(pathText('/home/me/proj/src/main.rs', false, cwd, home), 'src/main.rs');
  assert.equal(pathText('/home/me/proj/src', true, cwd, home), 'src/');
  assert.equal(pathText('/home/me/notes/a.md', false, cwd, home), '~/notes/a.md');
  assert.equal(pathText('/etc/hosts', false, cwd, home), '/etc/hosts');
  assert.equal(pathText('/home/me/proj/my docs/a.md', false, cwd, home), "'my docs/a.md'");
  assert.equal(pathText("/home/me/proj/it's.md", false, cwd, home), `'it'\\''s.md'`);
});

test('Tab 进目录：词换成这个目录（名字里的空格写成 \\ ），接着按目录找', () => {
  assert.equal(dirWord('src/'), '@src/');
  assert.equal(dirWord('my docs/'), '@my\\ docs/');
  assert.equal(dirWord('~/'), '@~/');
});

test('换掉框里的那个词：交回新的字和光标', () => {
  assert.deepEqual(splice('看看 @src/ma 吧', 3, 10, 'src/main.rs '), { value: '看看 src/main.rs  吧', caret: 15 });
  assert.deepEqual(splice('@ma', 0, 3, ''), { value: '', caret: 0 });
});

test('问不到的写清楚为什么：桥太旧（核心回「没有这个方法」）说重新编、重启桥；别的照原话（2026-10-01）', () => {
  assert.equal(failure({ code: -32601, message: 'Method not found' }), '桥太旧，列不了文件：重新编译、重启桥');
  assert.equal(failure({ code: -32010, message: '这是 Miyu 自己的数据，不列' }), '这是 Miyu 自己的数据，不列');
  assert.equal(failure(new Error('工作目录不在：/x')), '工作目录不在：/x');
});
