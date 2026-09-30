// @ts-check
//! 预览工作区里放什么（蓝图 `web.md`「预览工作区」第 1 条）：`write`、`edit` 改过、落在工作目录下 `artifacts/` 里的文件；
//! 撤销掉的那几轮的不算，删了的不算；一个文件一项，最后改的在前；类型照扩展名。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, ev } from './support.js';
import { artifacts, artifactKind } from '../src/model/artifacts.js';

loadRes();

const DIR = '/home/me/proj/artifacts';

/** 一条改了文件的结果。 */
const changed = (seq, turn, path) => ev(seq, seq, 'tool.result', turn, { call_id: `c${seq}`, status: 'ok', blocks: [], effects: [{ kind: 'file.changed', path, before: null, after: `sha256:${'a'.repeat(64)}` }] });
const trashed = (seq, turn, path) => ev(seq, seq, 'tool.result', turn, { call_id: `c${seq}`, status: 'ok', blocks: [], effects: [{ kind: 'file.trashed', path, trash: '/t' }] });

test('落在 artifacts/ 里改过的才算；一个文件一项，最后改的在前', () => {
  const log = [
    changed(1, 1, `${DIR}/report.md`),
    changed(2, 1, '/home/me/proj/src/main.rs'),
    changed(3, 2, `${DIR}/page.html`),
    changed(4, 3, `${DIR}/report.md`),
    changed(5, 3, '/home/me/proj/artifacts-old/x.md'),
  ];
  const list = artifacts(log, DIR);
  assert.deepEqual(list.map((a) => [a.path, a.seq]), [[`${DIR}/report.md`, 4], [`${DIR}/page.html`, 3]]);
  assert.deepEqual(list.map((a) => [a.name, a.kind]), [['report.md', 'markdown'], ['page.html', 'html']]);
});

test('子目录里的也算，名字写相对 artifacts/ 的路径', () => {
  assert.equal(artifacts([changed(1, 1, `${DIR}/charts/a.png`)], DIR)[0].name, 'charts/a.png');
});

test('撤销掉的那几轮的不算，恢复了又算', () => {
  const log = [changed(1, 1, `${DIR}/a.md`), changed(2, 2, `${DIR}/b.md`), ev(3, 3, 'turn.reverted', undefined, { turns: [2] })];
  assert.deepEqual(artifacts(log, DIR).map((a) => a.name), ['a.md']);
  log.push(ev(4, 4, 'turn.unreverted', undefined, { turns: [2] }));
  assert.deepEqual(artifacts(log, DIR).map((a) => a.name), ['b.md', 'a.md']);
});

test('删掉的不算；删了以后又写的算', () => {
  const log = [changed(1, 1, `${DIR}/a.md`), trashed(2, 2, `${DIR}/a.md`)];
  assert.deepEqual(artifacts(log, DIR), []);
  log.push(changed(3, 3, `${DIR}/a.md`));
  assert.equal(artifacts(log, DIR).length, 1);
});

test('不知道目录（桥还没回真实位置）的一项都没有', () => {
  assert.deepEqual(artifacts([changed(1, 1, `${DIR}/a.md`)], null), []);
});

test('类型照扩展名', () => {
  assert.deepEqual(['a.MD', 'b.htm', 'c.png', 'd.svg', 'e.mp4', 'f.rs', 'g.json', 'h.txt', 'i'].map(artifactKind),
    ['markdown', 'html', 'image', 'image', 'video', 'code', 'code', 'text', 'text']);
});
