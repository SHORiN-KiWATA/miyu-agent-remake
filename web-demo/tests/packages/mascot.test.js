// @ts-check
//! 吉祥物（软件包 `mascot`，蓝图 `web.md`「吉祥物」）：和 TUI 同一个模型，一格一个像素：正脸左右对称、有眼睛；闭眼没有眼睛；
//! 转头以后不对称；每一格照亮度分四档。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { render } from '../../packages/mascot/model.js';

const manifest = JSON.parse(readFileSync(new URL('../../packages/mascot/manifest.json', import.meta.url), 'utf8'));
const config = Object.fromEntries(Object.entries(manifest.settings).map(([k, s]) => [k, s.default]));
const look = config.model;
const pose = (extra = {}) => ({ yaw: 0, pitch: 0, blink: false, ear: 0, ...extra });
const size = { cols: config.cols, rows: config.rows, radius: config.radius, center_row: config.center_row };
const grid = (cells) => cells.map((row) => row.map((c) => (c ? `${c.part[0]}${c.level}` : '..')).join(' '));

test('正脸：左右对称，有眼睛、锯齿线（嘴，平常合着）、肚子上的圈、耳朵、鳍；每一格的亮度在四档里', () => {
  const cells = render(look, pose(), size);
  assert.equal(cells.length, config.rows);
  const flat = cells.flat().filter(Boolean);
  for (const part of ['head', 'ear', 'fin', 'eye', 'line', 'belly']) assert.ok(flat.some((c) => c.part === part), `有 ${part}`);
  assert.ok(!cells.flat().some((c) => c?.part === 'mouth'), '嘴平常合着：没有嘴里');
  assert.ok(flat.every((c) => c.level >= 0 && c.level <= 3));
  for (const row of cells) assert.deepEqual(row.map((c) => c?.part ?? null), [...row].reverse().map((c) => c?.part ?? null));
});

test('闭眼没有眼睛；转头以后不再左右对称', () => {
  assert.ok(!render(look, pose({ blink: true }), size).flat().some((c) => c?.part === 'eye'));
  const turned = render(look, pose({ yaw: 30 }), size);
  assert.notDeepEqual(grid(turned), grid(turned.map((row) => [...row].reverse())));
});

test('迈步（stride）：左右两片鳍一高一低，反过来迈就反过来；不迈的左右对称', () => {
  /** 某一块最下面那一格在第几行（左半边、右半边分开数） */
  const lowest = (cells, part, side) => {
    let row = -1;
    cells.forEach((r, y) => r.forEach((c, x) => {
      if (c?.part === part && (side < 0 ? x < config.cols / 2 : x >= config.cols / 2)) row = Math.max(row, y);
    }));
    return row;
  };
  const still = render(look, pose(), size);
  assert.equal(lowest(still, 'fin', -1), lowest(still, 'fin', 1));
  const left = render(look, pose({ stride: 1 }), size);
  assert.ok(lowest(left, 'fin', -1) < lowest(left, 'fin', 1), '左鳍抬起来：它最下面一格比右鳍的高');
  const right = render(look, pose({ stride: -1 }), size);
  assert.ok(lowest(right, 'fin', 1) < lowest(right, 'fin', -1), '反过来迈：右鳍抬起来');
});

test('蹲（crouch）：头往下沉、最下面一行（脚）不动；负的往上抻长', () => {
  /** 有东西的第一行、最后一行 */
  const span = (cells) => {
    const rows = cells.map((r, y) => (r.some(Boolean) ? y : -1)).filter((y) => y >= 0);
    return [rows[0], rows[rows.length - 1]];
  };
  const [top, bottom] = span(render(look, pose(), size));
  const [ctop, cbottom] = span(render(look, pose({ crouch: 1 }), size));
  assert.ok(ctop > top, `蹲下头顶往下：${top} → ${ctop}`);
  assert.equal(cbottom, bottom, '脚还踩在原地');
  const [stop] = span(render(look, pose({ crouch: -0.5 }), size));
  assert.ok(stop < top, `抻长头顶往上：${top} → ${stop}`);
  assert.ok(render(look, pose({ crouch: 1 }), size).flat().some((c) => c?.part === 'eye'), '蹲下了脸还在');
});

test('嘴是那道锯齿线：合着是一道线；张开时上下两排分开，中间是嘴里，张得越大嘴里越多', () => {
  const inside = (m) => render(look, pose({ mouth: m }), size).flat().filter((c) => c?.part === 'mouth').length;
  assert.equal(inside(0), 0, '合着');
  assert.ok(inside(0.5) > 0, '张开一点');
  assert.ok(inside(1) > inside(0.5), '张到最大');
});

test('肚子上的圈是纹路：一圈线，里面是身子（不是洞、不是深色），不跟着张嘴变', () => {
  const cells = render(look, pose(), size);
  const ring = [];
  cells.forEach((row, y) => row.forEach((c, x) => { if (c?.part === 'belly') ring.push([x, y]); }));
  assert.ok(ring.length > 0, '有那一圈');
  const xs = ring.map(([x]) => x);
  const ys = ring.map(([, y]) => y);
  const mid = [Math.round((Math.min(...xs) + Math.max(...xs)) / 2), Math.round((Math.min(...ys) + Math.max(...ys)) / 2)];
  assert.equal(cells[mid[1]][mid[0]]?.part, 'head', '圈里面是身子');
  const open = render(look, pose({ mouth: 1 }), size);
  const ringOpen = open.flat().filter((c) => c?.part === 'belly').length;
  assert.equal(ringOpen, ring.length, '张嘴不影响肚子上的圈');
});

test('侧身（turn）走路：整个转过去（鳍也跟着转）；迈步时往前那片鳍伸到前面去、抬起来，往后那片踩在地上', () => {
  /** 鳍的格子最右在第几列、最下在第几行 */
  const fin = (cells) => {
    let right = -1;
    let low = -1;
    cells.forEach((r, y) => r.forEach((c, x) => {
      if (c?.part !== 'fin') return;
      right = Math.max(right, x);
      low = Math.max(low, y);
    }));
    return { right, low };
  };
  const front = fin(render(look, pose(), size));
  const side = fin(render(look, pose({ turn: 80 }), size));
  assert.ok(side.right < front.right, `侧过去两片鳍前后叠着，没有正面那么宽：${front.right} → ${side.right}`);
  const forward = fin(render(look, pose({ turn: 80, stride: 1 }), size));
  const back = fin(render(look, pose({ turn: 80, stride: -1 }), size));
  assert.ok(forward.right > side.right, `往右走、迈步：前面那片伸到右边去：${side.right} → ${forward.right}`);
  assert.ok(back.right > side.right, '换一片迈也往前伸');
});

test('肚子上的灯（lamp）：亮着时圈里铺上灯，灭着圈里是身子；圈本身一直在', () => {
  const inside = (cells) => cells.flat().filter((c) => c?.part === 'lamp').length;
  assert.equal(inside(render(look, pose(), size)), 0, '灭着');
  const on = render(look, pose({ lamp: true }), size);
  assert.ok(inside(on) > 0, '亮着');
  assert.ok(on.flat().some((c) => c?.part === 'belly'), '圈还在');
});
