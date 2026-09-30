// @ts-check
//! 数的写法：和 TUI 演示的 `meter.rs` 一字不差（同几个例子），两个头写出来一样。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { short, hitRate, percentTenths, clock, seconds } from '../src/model/format.js';
import { bytes } from '../src/lib/format.js';

test('token 写短：一千以下照写，一千以上 k、一百万以上 M，一位小数，整的不写小数', () => {
  assert.equal(short(950), '950');
  assert.equal(short(1_000), '1k');
  assert.equal(short(12_345), '12.3k');
  assert.equal(short(1_000_000), '1M');
  assert.equal(short(1_400_000), '1.4M');
});

test('命中率：99 以下写整数，99 以上一位小数，正好 99.0 写 99，四舍五入到 100.0 写 100', () => {
  assert.equal(hitRate(94, 100), '94');
  assert.equal(hitRate(9_860, 10_000), '99', '98.6 还不到 99，照整数写');
  assert.equal(hitRate(9_900, 10_000), '99');
  assert.equal(hitRate(9_963, 10_000), '99.6');
  assert.equal(hitRate(9_990, 10_000), '99.9');
  assert.equal(hitRate(9_996, 10_000), '100');
  assert.equal(hitRate(0, 0), '0');
});

test('上下文的百分比：一位小数，整的不写小数', () => {
  assert.equal(percentTenths(2_283, 1_000_000), '0.2');
  assert.equal(percentTenths(500_000, 1_000_000), '50');
  assert.equal(percentTenths(1, 0), '0');
});

test('读秒：12s、1m 05s、1h 02m 05s', () => {
  assert.equal(clock(12), '12s');
  assert.equal(clock(65), '1m 05s');
  assert.equal(clock(3_725), '1h 02m 05s');
});

test('一轮的用时：一分钟以内带一位小数，再长照读秒', () => {
  assert.equal(seconds(11_840), '11.8s');
  assert.equal(seconds(24_100), '24.1s');
  assert.equal(seconds(65_000), '1m 05s');
});

test('文件大小：1024 进一级，一位小数，整的不写小数；一千零二十四以下写字节', () => {
  assert.equal(bytes(0), '0 B');
  assert.equal(bytes(812), '812 B');
  assert.equal(bytes(1024), '1 KB');
  assert.equal(bytes(1536), '1.5 KB');
  assert.equal(bytes(1024 * 1024 * 3.25), '3.3 MB');
  assert.equal(bytes(1024 ** 3 * 2), '2 GB');
});
