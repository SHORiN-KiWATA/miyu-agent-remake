// @ts-check
//! 分块上传（蓝图 `web.md`「附件」第 2 条，核心施工 W-5）：`blob.open`、一块一块 `blob.write`、`blob.close`；接不上的照 `data.received` 接着传。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { upload, toBase64, serial } from '../src/lib/upload.js';
import { Refusal } from '../src/core/connection.js';

test('base64：和 Node 的一样，长度不是 3 的倍数的补 =', () => {
  for (const n of [0, 1, 2, 3, 4, 5, 255, 1000]) {
    const bytes = Uint8Array.from({ length: n }, (_, i) => (i * 37 + 11) % 256);
    assert.equal(toBase64(bytes), Buffer.from(bytes).toString('base64'), String(n));
  }
});

/** 假的核心：记下每一次请求，照 `script` 改回应。 */
function core(size, script = {}) {
  const calls = [];
  let received = 0;
  const request = async (method, params) => {
    calls.push([method, method === 'blob.write' ? { ...params, data: Buffer.from(params.data, 'base64').length } : params]);
    const hook = script[`${method}#${calls.filter((c) => c[0] === method).length}`];
    if (hook) return hook(params, received, (n) => { received = n; });
    if (method === 'blob.open') return { upload: 'u1' };
    if (method === 'blob.write') {
      if (params.offset !== received) throw new Refusal('offset', -32010, 'upload_offset', { received });
      received += Buffer.from(params.data, 'base64').length;
      return { received };
    }
    if (method === 'blob.close') {
      if (received !== size) throw new Refusal('incomplete', -32010, 'upload_incomplete', { received });
      return { blob: 'sha256:x', name: 'a.png', media_type: 'image/png', kind: 'image' };
    }
    throw new Error(method);
  };
  return { calls, request };
}

const LIMITS = { chunk: 4, tries: 3 };
const file = (size) => ({ name: 'a.png', size, type: 'image/png' });
const read = async (offset, length) => new Uint8Array(Math.max(0, Math.min(length, 10 - offset)));

test('开、一块一块写、存好：交回 blob.close 的回应（和 blob.put 一样）', async () => {
  const { calls, request } = core(10);
  const got = await upload(request, file(10), 'image/png', read, LIMITS);
  assert.deepEqual(got, { blob: 'sha256:x', name: 'a.png', media_type: 'image/png', kind: 'image' });
  assert.deepEqual(calls, [
    ['blob.open', { name: 'a.png', size: 10, media_type: 'image/png' }],
    ['blob.write', { upload: 'u1', offset: 0, data: 4 }],
    ['blob.write', { upload: 'u1', offset: 4, data: 4 }],
    ['blob.write', { upload: 'u1', offset: 8, data: 2 }],
    ['blob.close', { upload: 'u1' }],
  ]);
});

test('媒体类型没有的不写；空文件：开了直接存', async () => {
  const { calls, request } = core(0);
  await upload(request, file(0), null, read, LIMITS);
  assert.deepEqual(calls, [['blob.open', { name: 'a.png', size: 0 }], ['blob.close', { upload: 'u1' }]]);
});

test('offset 接不上（upload_offset）：照 data.received 从那里接着传', async () => {
  const { calls, request } = core(10, {
    // 第二块写的时候核心说只收到 2 个（比如上一块只收了一半）
    'blob.write#2': (params, received, set) => { set(2); throw new Refusal('offset', -32010, 'upload_offset', { received: 2 }); },
  });
  await upload(request, file(10), 'image/png', read, LIMITS);
  assert.deepEqual(calls.filter((c) => c[0] === 'blob.write').map((c) => c[1].offset), [0, 4, 2, 6]);
});

test('存的时候没收齐（upload_incomplete）：接着传完再存', async () => {
  const { calls, request } = core(10, {
    'blob.close#1': (params, received, set) => { set(6); throw new Refusal('incomplete', -32010, 'upload_incomplete', { received: 6 }); },
  });
  const got = await upload(request, file(10), 'image/png', read, LIMITS);
  assert.equal(got.blob, 'sha256:x');
  assert.deepEqual(calls.map((c) => [c[0], c[1].offset ?? '']), [
    ['blob.open', ''], ['blob.write', 0], ['blob.write', 4], ['blob.write', 8], ['blob.close', ''], ['blob.write', 6], ['blob.close', ''],
  ]);
});

test('别的拒绝（太大、编号作废、连接断了）照原样抛；一直接不上的不无限重来', async () => {
  const big = { request: async () => { throw new Refusal('big', -32010, 'attachment_too_big'); } };
  await assert.rejects(upload(big.request, file(10), null, read, LIMITS), (e) => e instanceof Refusal && e.reason === 'attachment_too_big');
  const stuck = core(10, Object.fromEntries(Array.from({ length: 20 }, (_, i) => [`blob.write#${i + 1}`, () => { throw new Refusal('o', -32010, 'upload_offset', { received: 0 }); }])));
  await assert.rejects(upload(stuck.request, file(10), null, read, LIMITS), (e) => e instanceof Refusal && e.reason === 'upload_offset');
});

test('一个接一个传：前一个完了（成了、出错都算）才开下一个，核心同时开的上传有上限', async () => {
  const run = serial();
  const log = [];
  const job = (name, fail) => run(async () => {
    log.push(`start ${name}`);
    await new Promise((r) => setTimeout(r, 5));
    log.push(`end ${name}`);
    if (fail) throw new Error(name);
    return name;
  });
  const results = await Promise.allSettled([job('a'), job('b', true), job('c')]);
  assert.deepEqual(log, ['start a', 'end a', 'start b', 'end b', 'start c', 'end c']);
  assert.deepEqual(results.map((r) => r.status), ['fulfilled', 'rejected', 'fulfilled']);
});
