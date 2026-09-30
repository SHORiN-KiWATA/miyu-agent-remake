// @ts-check
//! 附件（软件包 `attachments`，蓝图 `web.md`「附件」）：收哪些、框里那一排的状态、发的时候交出什么、拒了放回来。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Tray, admit, mediaType, extOf, attachable } from '../../packages/attachments/model.js';

/** 这个包的设置项的出厂值 */
const config = Object.fromEntries(Object.entries(JSON.parse(readFileSync(new URL('../../packages/attachments/manifest.json', import.meta.url), 'utf8')).settings).map(([k, s]) => [k, s.default]));

/** 一个假文件：只要名字、大小、类型。 */
const file = (name, size = 10, type = '') => ({ name, size, type });

test('收哪些：超过上限大小的不收，放满了多的不收，别的照先后收（第 1、2 条）', () => {
  const big = file('大.mp4', config.max_mib * 1024 * 1024 + 1);
  const edge = file('正好.bin', config.max_mib * 1024 * 1024);
  const r = admit(0, [file('a.txt'), big, edge], config);
  assert.deepEqual(r.accepted.map((f) => f.name), ['a.txt', '正好.bin']);
  assert.deepEqual(r.tooBig.map((f) => f.name), ['大.mp4']);
  assert.equal(r.tooMany, 0);
  const full = admit(config.max_files - 1, [file('a'), file('b'), file('c')], config);
  assert.deepEqual(full.accepted.map((f) => f.name), ['a']);
  assert.equal(full.tooMany, 2);
});

test('媒体类型：合规矩的照写（小写），空的、不合的不写，交给核心照内容认（第 2 条）', () => {
  assert.equal(mediaType('image/png'), 'image/png');
  assert.equal(mediaType('Application/PDF'), 'application/pdf');
  assert.equal(mediaType('application/vnd.openxmlformats-officedocument.wordprocessingml.document'), 'application/vnd.openxmlformats-officedocument.wordprocessingml.document');
  assert.equal(mediaType(''), null);
  assert.equal(mediaType('text/plain; charset=utf-8'), null);
  assert.equal(mediaType('image/png/x'), null);
  assert.equal(mediaType(`a/${'b'.repeat(130)}`), null);
});

test('卡左下角的标签：扩展名大写；没有扩展名的不写（第 3 条）', () => {
  assert.equal(extOf('报告.pdf'), 'PDF');
  assert.equal(extOf('archive.tar.gz'), 'GZ');
  assert.equal(extOf('Makefile'), '');
  assert.equal(extOf('.bashrc'), '');
});

test('框里那一排：传的时候算在准备，不能发；传完了能发；拿掉的传完了也不算（第 3、4 条）', () => {
  const tray = new Tray();
  assert.equal(tray.has(), false);
  const a = tray.add(file('a.png', 10, 'image/png'));
  const b = tray.add(file('b.txt'));
  assert.deepEqual(tray.items.map((it) => it.state), ['uploading', 'uploading']);
  assert.equal(tray.has(), true);
  assert.equal(tray.busy(), true);
  assert.equal(tray.ready(a.id, { blob: 'sha256:a', name: 'a.png', media_type: 'image/png', kind: 'image' }), true);
  assert.equal(tray.busy(), true);
  tray.remove(b.id);
  assert.equal(tray.ready(b.id, { blob: 'sha256:b', name: 'b.txt', media_type: 'text/plain', kind: 'file' }), false, '拿掉以后才传完的不收');
  assert.equal(tray.busy(), false);
  assert.deepEqual(tray.items.map((it) => it.name), ['a.png']);
});

test('发：交出传完的几个（只要 blob、名字、媒体类型），框里清空；拒了放回来，照原来的先后（第 4 条）', () => {
  const tray = new Tray();
  const a = tray.add(file('a.png'));
  const b = tray.add(file('b.pdf'));
  tray.ready(a.id, { blob: 'sha256:a', name: 'a.png', media_type: 'image/png', kind: 'image', width: 2, height: 1 });
  tray.ready(b.id, { blob: 'sha256:b', name: 'b.pdf', media_type: 'application/pdf', kind: 'file' });
  const taken = tray.take();
  assert.deepEqual(taken, { attachments: [
    { blob: 'sha256:a', name: 'a.png', media_type: 'image/png' },
    { blob: 'sha256:b', name: 'b.pdf', media_type: 'application/pdf' },
  ] });
  assert.equal(tray.has(), false);
  const c = tray.add(file('c.txt'));
  tray.ready(c.id, { blob: 'sha256:c', name: 'c.txt', media_type: 'text/plain', kind: 'file' });
  assert.ok(taken);
  tray.putBack(taken);
  assert.deepEqual(tray.items.map((it) => it.name), ['a.png', 'b.pdf', 'c.txt'], '放回来的在前面');
  assert.equal(tray.items[0].state, 'ready');
});

test('没有东西、还在传的时候不交；不是自己交出去的不收回', () => {
  const tray = new Tray();
  assert.equal(tray.take(), null);
  tray.add(file('a'));
  assert.equal(tray.take(), null);
  tray.putBack({ attachments: [{ blob: 'sha256:x', name: 'x', media_type: 'text/plain' }] });
  assert.equal(tray.items.length, 1);
});

test('变了告诉看着的：加、传完、拿掉、交出、放回都算', () => {
  const tray = new Tray();
  let n = 0;
  tray.watch(() => n++);
  const a = tray.add(file('a'));
  tray.ready(a.id, { blob: 'sha256:a', name: 'a', media_type: 'text/plain', kind: 'file' });
  const taken = tray.take();
  if (taken) tray.putBack(taken);
  tray.remove(a.id);
  assert.equal(n, 5);
});

test('按种类选小卡的图标、要不要缩略图：图片、视频有缩略图；音频、PDF、文字、别的各一个图标', async () => {
  const { kindOf } = await import('../../packages/attachments/model.js');
  assert.equal(kindOf({ name: 'a.png', type: 'image/png' }), 'image');
  assert.equal(kindOf({ name: 'a.mp4', type: 'video/mp4' }), 'video');
  assert.equal(kindOf({ name: 'a.mp3', type: 'audio/mpeg' }), 'audio');
  assert.equal(kindOf({ name: 'a.pdf', type: 'application/pdf' }), 'pdf');
  assert.equal(kindOf({ name: 'notes.txt', type: 'text/plain' }), 'text');
  assert.equal(kindOf({ name: 'x.bin', type: '' }), 'file');
});

test('文字文件有几行：最后一行没有换行也算一行；空的是 0 行', async () => {
  const { lineCount } = await import('../../packages/attachments/model.js');
  assert.equal(lineCount('a\nb\nc'), 3);
  assert.equal(lineCount('a\nb\n'), 2);
  assert.equal(lineCount(''), 0);
});

test('输入历史：交出去的记成核心存好的那一份（编号、名字、媒体类型、大小），浏览器里的文件不记', () => {
  const tray = new Tray();
  const a = tray.add({ name: 'a.png', size: 3, type: 'image/png', file: new Blob(['abc']) });
  tray.ready(a.id, { blob: 'sha256:aa', name: 'a.png', media_type: 'image/png' });
  const taken = /** @type {any} */ (tray.take());
  assert.deepEqual(tray.keep(taken), [{ blob: 'sha256:aa', name: 'a.png', media_type: 'image/png', size: 3 }]);
  assert.equal(tray.keep(/** @type {any} */ ({ attachments: [] })), null, '不是这里交出去的不认');
});

test('输入历史翻出来的：换掉上一次跟着翻出来的，自己放的不动；传好了的直接能发；文件带着核心存好的那一份（缩略图照它取）；留下以后不再换', () => {
  const tray = new Tray();
  const own = tray.add({ name: 'mine.txt', size: 1, type: 'text/plain' });
  tray.ready(own.id, { blob: 'sha256:00', name: 'mine.txt', media_type: 'text/plain' });
  const kept = [{ blob: 'sha256:aa', name: 'a.png', media_type: 'image/png', size: 3 }];
  tray.recall(kept, 's1');
  assert.deepEqual(tray.items.map((it) => it.name), ['mine.txt', 'a.png']);
  assert.equal(tray.busy(), false);
  assert.deepEqual(tray.items[1].file, { name: 'a.png', size: 3, type: 'image/png', stored: { session: 's1', hash: 'sha256:aa' } });
  tray.recall([{ blob: 'sha256:bb', name: 'b.pdf', media_type: 'application/pdf', size: 9 }], 's2');
  assert.deepEqual(tray.items.map((it) => it.name), ['mine.txt', 'b.pdf'], '换掉上一次跟着翻出来的');
  tray.recall(null, null);
  assert.deepEqual(tray.items.map((it) => it.name), ['mine.txt'], '走回没发的那句：拿掉');
  tray.recall(kept, 's1');
  tray.settle();
  tray.recall(null, null);
  assert.deepEqual(tray.items.map((it) => it.name), ['mine.txt', 'a.png'], '留下的不再拿掉');
  assert.deepEqual(/** @type {any} */ (tray.take()).attachments.map((x) => x.blob), ['sha256:00', 'sha256:aa']);
});

test('@ 选文件交过来的：图片、PDF、音频、视频收成附件，别的（文字、代码、目录）不收，照路径写进话里', () => {
  const ref = (name, type) => ({ name, size: 1, type, path: `/p/${name}` });
  assert.equal(attachable(ref('a.png', 'image/png')), true);
  assert.equal(attachable(ref('b.pdf', 'application/pdf')), true);
  assert.equal(attachable(ref('c.mp3', 'audio/mpeg')), true);
  assert.equal(attachable(ref('d.mp4', 'video/mp4')), true);
  assert.equal(attachable(ref('e.rs', 'text/plain; charset=utf-8')), false);
  assert.equal(attachable(ref('f.zip', 'application/zip')), false);
});

test('键盘拿掉：光标在最前面按退格，拿掉最后一张（在传的也能拿）；一张都没有的交回 false，退格照常', () => {
  const tray = new Tray();
  const a = tray.add({ name: 'a.png', size: 1, type: 'image/png' });
  tray.ready(a.id, { blob: 'sha256:aa', name: 'a.png', media_type: 'image/png' });
  tray.add({ name: 'b.pdf', size: 1, type: 'application/pdf' });
  assert.equal(tray.dropLast(), true);
  assert.deepEqual(tray.items.map((it) => it.name), ['a.png']);
  assert.equal(tray.dropLast(), true);
  assert.equal(tray.dropLast(), false);
});
