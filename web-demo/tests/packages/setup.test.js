// @ts-check
//! 人格、工作区（软件包 setup，蓝图 web.md「人格、预设、工作区」）：按钮上写什么、默认的人格能不能用、路径怎么认、最近用过的。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { personaName, presetName, defaultUsable, presetInUse, personaInUse, dirName, readPath, remember, tilde } from '../../packages/setup/model.js';
import { sessionCwd } from '../../src/model/session.js';

const list = [{ persona: 'engineer', name: null }, { persona: 'miyu', name: 'Miyu' }, { persona: 'bad', problem: 'persona.toml:2: …' }];

test('人格的名字：有显示名的写显示名，没有的写编号', () => {
  assert.equal(personaName(list[0]), 'engineer');
  assert.equal(personaName(list[1]), 'Miyu');
});

test('默认的人格能不能用：在列表里、没写错才算；没设、指着没有的、写错的都锁住（第 2 条）', () => {
  assert.equal(defaultUsable(list, 'engineer'), true);
  assert.equal(defaultUsable(list, 'nope'), false);
  assert.equal(defaultUsable(list, 'bad'), false);
  assert.equal(defaultUsable(list, null), false);
});

test('路径：只认 ~、~/…、/…，去掉前后空白和末尾的 /；相对的不认', () => {
  assert.equal(readPath('  ~/src/miyu/ '), '~/src/miyu');
  assert.equal(readPath('~'), '~');
  assert.equal(readPath('/'), '/');
  assert.equal(readPath('src/miyu'), null);
  assert.equal(readPath(''), null);
});

test('路径在 Windows 上的写法也认（核心在哪个系统上都行）：C:\\…、C:/…、\\\\服务器\\…，盘符的根留着分隔符', () => {
  assert.equal(readPath('C:\\Users\\a\\'), 'C:\\Users\\a');
  assert.equal(readPath('C:\\'), 'C:\\');
  assert.equal(readPath('D:/work/'), 'D:/work');
  assert.equal(readPath('\\\\srv\\share'), '\\\\srv\\share');
  assert.equal(dirName('C:\\Users\\a'), 'a');
  assert.equal(tilde('C:\\Users\\a\\src', 'C:\\Users\\a'), '~\\src');
});

test('按钮上写目录名；家目录下的写成 ~/…', () => {
  assert.equal(dirName('~/Documents/github/Miyu'), 'Miyu');
  assert.equal(dirName('~'), '~');
  assert.equal(dirName('/'), '/');
  assert.equal(tilde('/home/a/src', '/home/a'), '~/src');
  assert.equal(tilde('/home/a', '/home/a'), '~');
  assert.equal(tilde('/home/ab', '/home/a'), '/home/ab');
});

test('最近用过的：排到最前、去重、最多几个，默认工作区不记', () => {
  assert.deepEqual(remember(['/b', '/a'], '/a', '/w', 3), ['/a', '/b']);
  assert.deepEqual(remember(['/a', '/b', '/c'], '/d', '/w', 3), ['/d', '/a', '/b']);
  assert.deepEqual(remember(['/a'], '/w', '/w', 3), ['/a']);
});

test('会话在哪干活：最后一条带 cwd 的 turn.started，没有的照 session.created', () => {
  const created = { kind: 'session.created', body: { cwd: '/w' } };
  assert.equal(sessionCwd([created]), '/w');
  assert.equal(sessionCwd([created, { kind: 'turn.started', body: { cwd: '~/a' } }, { kind: 'turn.started', body: {} }]), '~/a');
  assert.equal(sessionCwd([]), null);
});

test('预设：名字照显示名、没有的写编号；默认的能不能用照 preset 那一格认（P-2）', () => {
  const presets = [{ preset: 'dev', name: '开发' }, { preset: 'full', name: null }, { preset: 'bad', problem: 'x' }];
  assert.equal(presetName(presets[0]), '开发');
  assert.equal(presetName(presets[1]), 'full');
  assert.equal(defaultUsable(presets, 'dev', 'preset'), true);
  assert.equal(defaultUsable(presets, 'bad', 'preset'), false);
  assert.equal(defaultUsable(presets, 'nope', 'preset'), false);
});

test('没选时实际用哪个：预设照 preset.default、没写是 full；人格照选的 → 预设写的默认人格 → persona.default → engineer', () => {
  assert.equal(presetInUse('dev', 'full'), 'dev');
  assert.equal(presetInUse(null, 'dev'), 'dev');
  assert.equal(presetInUse(null, null), 'full');
  assert.equal(personaInUse('miyu', 'engineer', 'none'), 'miyu');
  assert.equal(personaInUse(null, 'engineer', 'none'), 'engineer');
  assert.equal(personaInUse(null, null, 'none'), 'none');
  assert.equal(personaInUse(null, null, null), 'engineer');
});
