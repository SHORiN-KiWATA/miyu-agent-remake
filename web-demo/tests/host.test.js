// @ts-check
//! 宿主（蓝图 `web/architecture.md`「宿主」「多用户、多终端」）：平台的 API 只在 `src/host/` 里用；浏览器那一份的地址带口令、
//! 下载带名字；这台设备上存的东西按账号分开；连核心只认一条「像 WebSocket」的线，桌面端换一条线就能用。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { urls } from '../src/host/browser.js';
import { accountStorage } from '../src/kernel/storage.js';
import { Connection, Refusal } from '../src/core/connection.js';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** 目录下全部 `.js`。 */
function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? files(p) : p.endsWith('.js') ? [p] : [];
  });
}

/** 平台的 API、写死的桥的地址（「宿主」的「页面自己守的」第 1 条） */
const PLATFORM = [
  /new WebSocket\b/, /\blocation\.hash\b/, /\blocalStorage\b/, /\bsessionStorage\b/, /\bwindow\.open\(/, /\bnavigator\.clipboard\b/,
  /\bdataTransfer\b/, /['"`]\/(ws|upload)\b/, /['"`]\/(file|blob|link-image)\?/,
];

test('平台的 API、桥的地址只在 src/host/ 里用', () => {
  const host = join(ROOT, 'src', 'host');
  const bad = [];
  for (const file of [...files(join(ROOT, 'src')), ...files(join(ROOT, 'packages'))]) {
    if (file.startsWith(host)) continue;
    readFileSync(file, 'utf8').split('\n').forEach((line, i) => {
      const code = line.replace(/\/\/.*$/, '');
      if (/^\s*\*/.test(line)) return;
      for (const re of PLATFORM) if (re.test(code)) bad.push(`${relative(ROOT, file)}:${i + 1} ${re}`);
    });
  }
  assert.deepEqual(bad, []);
});

test('浏览器那一份的地址：带口令，不带会话（核心照账号、照路径给，W-6）；下载的带 download，blob 另带原来的名字', () => {
  const u = urls('k1');
  const file = new URL(u.file('/home/a b/x.png'), 'http://x');
  assert.equal(file.pathname, '/file');
  assert.deepEqual(Object.fromEntries(file.searchParams), { k: 'k1', path: '/home/a b/x.png' });
  assert.equal(new URL(u.file('/x', true), 'http://x').searchParams.get('download'), '1');
  const blob = new URL(u.blob('sha256:ab', 'application/pdf', { download: true, name: '报告.pdf' }), 'http://x');
  assert.equal(blob.pathname, '/blob');
  assert.deepEqual(Object.fromEntries(blob.searchParams), { k: 'k1', hash: 'sha256:ab', type: 'application/pdf', download: '1', name: '报告.pdf' });
  assert.equal(new URL(u.blob('sha256:ab', 'image/png'), 'http://x').searchParams.has('name'), false, '不下载的不带名字');
  assert.equal('linkImage' in u, false, '链接卡片的图是 blob，照 /blob 取（核心施工 W-7）');
});

/** 一个假的存法：一张表，`broken` 时读写都抛（隐私窗口、清过数据） */
function fakeStore(broken = false) {
  const table = new Map();
  return {
    table,
    getItem: (k) => { if (broken) throw new Error('no'); return table.get(k) ?? null; },
    setItem: (k, v) => { if (broken) throw new Error('no'); table.set(k, v); },
    removeItem: (k) => { table.delete(k); },
  };
}

test('这台设备上存的按账号分开：键带账号，换一个账号看不到（「多用户、多终端」第 5 条）', () => {
  const store = fakeStore();
  const admin = accountStorage(store, 'admin');
  admin.set('packages', { rail: { disabled: true } });
  assert.equal(store.table.get('miyu.admin.packages'), '{"rail":{"disabled":true}}');
  assert.deepEqual(admin.get('packages', {}), { rail: { disabled: true } });
  assert.deepEqual(accountStorage(store, 'alice').get('packages', {}), {});
});

test('存法读写抛错、存的不是 JSON 的，照没有算，不让页面起不来', () => {
  assert.equal(accountStorage(fakeStore(true), 'admin').get('x', 7), 7);
  accountStorage(fakeStore(true), 'admin').set('x', 1);
  const store = fakeStore();
  store.table.set('miyu.admin.x', '{坏的');
  assert.equal(accountStorage(store, 'admin').get('x', 7), 7);
});

test('原来不分账号的旧键，第一次读时搬到这个账号名下', () => {
  const store = fakeStore();
  store.table.set('miyu.packages', '{"todo":{"disabled":true}}');
  const admin = accountStorage(store, 'admin', { packages: 'miyu.packages' });
  assert.deepEqual(admin.get('packages', {}), { todo: { disabled: true } });
  assert.equal(store.table.has('miyu.packages'), false);
  assert.equal(store.table.get('miyu.admin.packages'), '{"todo":{"disabled":true}}');
});

/** 一条假的线：样子照 WebSocket（`send`、`readyState`、`onopen`、`onmessage`、`onclose`、`onerror`） */
function fakeChannel() {
  const ch = { readyState: 0, sent: /** @type {any[]} */ ([]), onopen: null, onmessage: null, onclose: null, onerror: null,
    send(text) { this.sent.push(JSON.parse(text)); } };
  return ch;
}

test('连核心只认一条像 WebSocket 的线：通了才发，回应对上请求，拒绝是 Refusal，断了等着的都拒掉', async () => {
  const ch = /** @type {any} */ (fakeChannel());
  const conn = new Connection(() => ch);
  const states = [];
  conn.onStatus((s) => states.push(s));
  const up = conn.connect();
  await assert.rejects(conn.request('x'), (e) => e instanceof Refusal && e.reason === 'disconnected');
  ch.readyState = 1;
  ch.onopen();
  await up;
  const got = conn.request('session.list', {});
  const [m] = ch.sent;
  assert.equal(m.method, 'session.list');
  ch.onmessage({ data: JSON.stringify({ jsonrpc: '2.0', id: m.id, result: { ok: 1 } }) });
  assert.deepEqual(await got, { ok: 1 });
  const refused = conn.request('session.send', {});
  ch.onmessage({ data: JSON.stringify({ jsonrpc: '2.0', id: ch.sent[1].id, error: { code: -32010, message: '不行', data: { reason: 'turn_running' } } }) });
  await assert.rejects(refused, (e) => e instanceof Refusal && e.reason === 'turn_running');
  const pending = conn.request('slow', {});
  ch.readyState = 3;
  ch.onclose();
  await assert.rejects(pending, (e) => e.reason === 'disconnected');
  assert.deepEqual(states, ['connecting', 'online', 'offline']);
});

test('断了自己重连：隔一会儿再开一条线，连上了告诉外面（重新握手、补上漏掉的由外面做）；一开始就连不上的不在这里重连', async () => {
  const lines = /** @type {any[]} */ ([]);
  const conn = new Connection(() => { const ch = fakeChannel(); lines.push(ch); return ch; }, [0, 0]);
  const states = [];
  conn.onStatus((s) => states.push(s));
  let reopened = 0;
  conn.onReopen(() => { reopened += 1; });
  const up = conn.connect();
  lines[0].readyState = 1;
  lines[0].onopen();
  await up;
  // 核心重启：桥把线关了
  lines[0].readyState = 3;
  lines[0].onclose();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(lines.length, 2, '又开了一条');
  // 这一次也没连上（核心还没起来）：接着试
  lines[1].onerror();
  lines[1].onclose();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(lines.length, 3, '再试一次');
  lines[2].readyState = 1;
  lines[2].onopen();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(reopened, 1, '连上了告诉外面一次');
  assert.deepEqual(states, ['connecting', 'online', 'offline', 'connecting', 'offline', 'connecting', 'online']);
});

test('重连时口令用不了了（桥重启过）：不再白试，告诉外面一声（对话区顶上挂提示）；口令还对的照常接着试', async () => {
  const lines = /** @type {any[]} */ ([]);
  let key = 'ok';
  const conn = new Connection(() => { const ch = fakeChannel(); lines.push(ch); return ch; }, [0, 0], async () => key);
  let lost = 0;
  conn.onLost(() => { lost += 1; });
  const up = conn.connect();
  lines[0].readyState = 1;
  lines[0].onopen();
  await up;
  lines[0].readyState = 3;
  lines[0].onclose();
  await new Promise((r) => setTimeout(r, 5));
  lines[1].onerror();
  lines[1].onclose();
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(lines.length, 3, '口令还对：接着试');
  key = 'bad';
  lines[2].onerror();
  lines[2].onclose();
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(lost, 1, '口令用不了了：说一声');
  assert.equal(lines.length, 3, '不再试');
});
