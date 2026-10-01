// @ts-check
//! `@` 选文件问核心（蓝图 `web.md`「`@` 选文件」第 2 条，核心施工 W-2）：按目录找经 `fs.list`、模糊找经 `fs.find`，都带工作目录；
//! 清单没建完（`building`）先交已经建好的，隔一会儿再问；媒体类型照扩展名认（核心的列表里不带）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { res } from '../src/util/res.js';
import { listFiles } from '../src/core/files.js';

loadRes();

/** 照顺序一条条回的假连接，记下问了什么。 */
function fakeConn(replies) {
  const asked = [];
  return { asked, request: async (method, params) => { asked.push([method, params]); return replies.shift(); } };
}

test('按目录找：fs.list 带工作目录、目录、开头；算「只读这一层」；文件照扩展名补上媒体类型，目录、认不得的不写', async () => {
  const conn = fakeConn([{ items: [{ path: 'docs/', full: '/w/docs', dir: true, marks: [] }, { path: 'a.PNG', full: '/w/a.PNG', dir: false, marks: [0], size: 3 }, { path: 'b.rs', full: '/w/b.rs', dir: false, marks: [], size: 9 }], partial: false }]);
  const found = await listFiles(/** @type {any} */ (conn), '/w', { mode: 'dir', dir: '', prefix: 'a' }, true);
  assert.deepEqual(conn.asked, [['fs.list', { cwd: '/w', dir: '', prefix: 'a' }]]);
  assert.equal(found.layer, true);
  assert.equal(found.partial, false);
  assert.deepEqual(found.items.map((x) => x.type), ['', 'image/png', '']);
});

test('模糊找：fs.find 带工作目录、打的字、开列表时的 fresh；没建完的先交出来，隔 find_poll_ms 再问（不再带 fresh），建完为止', async () => {
  const saved = res.layout.find_poll_ms;
  res.layout.find_poll_ms = 1;
  try {
    const conn = fakeConn([
      { items: [{ path: 'main.rs', full: '/w/main.rs', dir: false, marks: [0] }], partial: false, building: true },
      { items: [{ path: 'main.rs', full: '/w/main.rs', dir: false, marks: [0] }, { path: 'src/main.js', full: '/w/src/main.js', dir: false, marks: [4] }], partial: false, building: false },
    ]);
    const seen = [];
    const found = await listFiles(/** @type {any} */ (conn), '/w', { mode: 'find', query: 'main' }, true, (f) => seen.push(f.items.length));
    assert.deepEqual(conn.asked, [['fs.find', { cwd: '/w', query: 'main', fresh: true }], ['fs.find', { cwd: '/w', query: 'main', fresh: false }]]);
    assert.deepEqual(seen, [1]);
    assert.equal(found.layer, false);
    assert.equal(found.items.length, 2);
  } finally {
    res.layout.find_poll_ms = saved;
  }
});

test('还要不要接着问由问的那一头定：不要了就停，交回最后一次的', async () => {
  const conn = fakeConn([{ items: [], partial: false, building: true }, { items: [], partial: false, building: false }]);
  const found = await listFiles(/** @type {any} */ (conn), '/w', { mode: 'find', query: 'x' }, false, () => {}, () => true);
  assert.equal(conn.asked.length, 1);
  assert.deepEqual(found.items, []);
});
