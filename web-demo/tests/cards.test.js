// @ts-check
//! 回答里的地址（蓝图 `web.md`「读本机文件」「音视频、图片卡片」）：本机路径换成绝对路径；单独一行的媒体地址认成卡片。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { localPath } from '../src/model/paths.js';
import { mediaLine, embedKind } from '../src/model/cards.js';

loadRes();

const WHERE = { home: '/home/me', cwd: '/home/me/proj' };

test('本机路径：绝对路径、~/、file:// 照写，相对路径照工作目录，网上的地址不算', () => {
  assert.equal(localPath('/tmp/a.png', WHERE), '/tmp/a.png');
  assert.equal(localPath('~/pics/a.png', WHERE), '/home/me/pics/a.png');
  assert.equal(localPath('file:///tmp/%E5%9B%BE.png', WHERE), '/tmp/图.png');
  assert.equal(localPath('out/a.png', WHERE), '/home/me/proj/out/a.png');
  assert.equal(localPath('./out/../b.png', WHERE), '/home/me/proj/b.png');
  assert.equal(localPath('https://x.com/a.png', WHERE), null);
  assert.equal(localPath('mailto:a@b.c', WHERE), null);
  assert.equal(localPath('', WHERE), null);
  // 不知道家目录的，~/ 不认
  assert.equal(localPath('~/a.png', { home: null, cwd: '/p' }), null);
});

test('单独一行的媒体地址：光地址、[说明](地址)，照扩展名分视频、音频、图片', () => {
  assert.deepEqual(mediaLine('/tmp/clip.MP4'), { kind: 'video', target: '/tmp/clip.MP4', label: 'clip.MP4' });
  assert.deepEqual(mediaLine('  [演示](~/a/demo.webm)  '), { kind: 'video', target: '~/a/demo.webm', label: '演示' });
  assert.deepEqual(mediaLine('https://x.com/song.mp3?t=1'), { kind: 'audio', target: 'https://x.com/song.mp3?t=1', label: 'song.mp3' });
  assert.equal(mediaLine('out/chart.png').kind, 'image');
});

test('不是单独一行的、不认得扩展名的、句子里的，都不是卡片', () => {
  assert.equal(mediaLine('看这个 /tmp/clip.mp4'), null);
  assert.equal(mediaLine('/tmp/notes.md'), null);
  assert.equal(mediaLine('[说明](/tmp/a.mp4) 后面还有字'), null);
  assert.equal(mediaLine('```'), null);
  assert.equal(mediaLine(''), null);
});

test('![说明](地址) 照扩展名：图片、视频、音频各画各的；没有扩展名的照图；别的扩展名（网页、文档）不是图，交 null 照链接写（2026-10-01）', () => {
  assert.equal(embedKind('out/chart.PNG'), 'image');
  assert.equal(embedKind('https://x.com/pic?id=3'), 'image');
  assert.equal(embedKind('~/a/demo.webm'), 'video');
  assert.equal(embedKind('song.mp3#t=1'), 'audio');
  assert.equal(embedKind('pelican-bicycle-anim.html'), null);
  assert.equal(embedKind('/tmp/notes.md'), null);
});
