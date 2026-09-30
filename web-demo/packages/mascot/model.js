// @ts-check
//! 吉祥物的模型和逐格打光线（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 1 条）：和 TUI 首页的吉祥物是同一个模型、同一种算法
//! （`tui-demo/src/mascot/render.rs`，蓝图 `tui.md`「空会话的首页」第 5 条），换成一格一个像素：TUI 照亮度挑字，这里照亮度分档，
//! 画的时候取这一块的颜色和它的暗色之间的一档。纯函数，不碰 DOM。
//!
//! 长度都以头的半径为 1：x 往右、y 往上、z 朝着看的人。角度是度，左右 `yaw` 往右为正，上下 `pitch` 往下为正。

/**
 * @typedef {'head'|'ear'|'fin'} Part
 * @typedef {{part: Part, center: number[], radii: number[], tilt: number, mirror?: boolean, follow?: number}} Shape
 * @typedef {{eyes: number[][][], eye_width: number, line: number[][], line_width: number, mouth_gap: number,
 *   belly: {center: number[], half: number[], round: number, ring: number}}} Face 脸：两只眼睛；锯齿线是嘴，张到最大上下两排分开
 *   `mouth_gap`；肚子上的圈是纹路（一圈宽 `ring` 的线）
 * @typedef {{shapes: Shape[], face: Face, light: number[], ambient: number, levels: number, stride_lift?: number, stride_swing?: number,
 *   crouch_drop?: number, crouch_squish?: number}} Look 迈步时鳍抬多高、前后迈多远（`stride_lift`、`stride_swing`）；蹲到底头往下沉
 *   多少、压扁多少（头的半径为 1）
 * @typedef {{yaw: number, pitch: number, blink: boolean, ear: number, stride?: number, crouch?: number, mouth?: number, turn?: number,
 *   lamp?: boolean}} Pose
 *   这一帧：头朝哪、闭没闭眼、耳朵往外多歪（弧度）、迈到哪（-1 到 1：正的左鳍往前迈、抬起来，负的右鳍）、蹲多深（0 到 1；负的往上
 *   抻长）、嘴张多大（0 合着、1 张到最大）、整个身子转多少度（侧身走路；鳍也跟着转，`yaw` 是头在这上面再看哪）、肚子上的灯亮没亮
 * @typedef {{part: Part|'eye'|'line'|'mouth'|'belly'|'lamp', level: number}} Cell 一格：哪一块、第几档亮（0 最暗）；`line` 是嘴那道锯齿线，
 *   `mouth` 是张开的嘴里，`belly` 是肚子上那盏灯的圈，`lamp` 是灯亮着时圈里
 */

/** 一格里四根光线的落点：左右、上下都对称，正脸画出来才左右对称。 */
const SAMPLES = [[0.25, 0.25], [0.75, 0.25], [0.25, 0.75], [0.75, 0.75]];
/** 四根里几根落在同一样上才算它（轮廓照它定，像素边不毛） */
const ENOUGH = 2;

/**
 * 摆成 `pose` 时画成的格子：`rows` 行、每行 `cols` 格；没打中的是 `null`。
 * @param {Look} look
 * @param {Pose} pose
 * @param {{cols: number, rows: number, radius: number, center_row: number}} grid 多少格、头的半径占几格、头的中心在第几行
 * @returns {(Cell|null)[][]}
 */
export function render(look, pose, grid) {
  const head = turn(pose.yaw + (pose.turn ?? 0), pose.pitch);
  const pieces = placed(look, pose);
  const bend = crouchOf(look, pose);
  const light = normalize(look.light);
  const half = grid.cols / 2;
  const top = look.levels - 1;
  const out = [];
  for (let r = 0; r < grid.rows; r++) {
    const row = [];
    for (let c = 0; c < grid.cols; c++) {
      const tally = { parts: { head: 0, ear: 0, fin: 0 }, lum: 0, lit: 0, eye: 0, line: 0, mouth: 0, belly: 0, lamp: 0 };
      for (const [dx, dy] of SAMPLES) {
        const x = (c + dx - half) / grid.radius;
        const y = -(r + dy - grid.center_row) / grid.radius;
        const hit = shoot(pieces, head, x, y);
        if (!hit) continue;
        // 蹲下以后头沉下去、压扁了：打中的那一点换回头原来的坐标再对脸
        const p = hit.point;
        const local = [p[0] / bend.sx, (p[1] + bend.drop) / bend.sy, p[2]];
        const mark = hit.part === 'head' ? onFace(look.face, local, pose.blink, pose.mouth ?? 0, !!pose.lamp) : null;
        if (mark) {
          tally[mark] += 1;
          continue;
        }
        tally.parts[hit.part] += 1;
        tally.lit += 1;
        tally.lum += look.ambient + (1 - look.ambient) * Math.max(0, dot(hit.normal, light));
      }
      row.push(cellOf(tally, look.ambient, top));
    }
    out.push(row);
  }
  return out;
}

/** 定这一格：眼睛、嘴、锯齿线够两根的照它；别的照打中的那几根的平均亮度分档，打中得太少的不画。 */
function cellOf(tally, ambient, top) {
  if (tally.eye >= ENOUGH) return { part: 'eye', level: top };
  if (tally.mouth >= ENOUGH) return { part: 'mouth', level: 0 };
  if (tally.line >= ENOUGH) return { part: 'line', level: 0 };
  if (tally.belly >= ENOUGH) return { part: 'belly', level: 0 };
  if (tally.lamp >= ENOUGH) return { part: 'lamp', level: top };
  if (tally.lit + tally.eye + tally.line + tally.mouth + tally.belly + tally.lamp < ENOUGH || !tally.lit) return null;
  const bright = (tally.lum / tally.lit - ambient) / (1 - ambient);
  const level = Math.max(0, Math.min(top, Math.round(bright * top)));
  const order = /** @type {Part[]} */ (['head', 'ear', 'fin']);
  const part = order.reduce((best, p) => (tally.parts[p] > tally.parts[best] ? p : best), order[0]);
  return { part, level };
}

/** 照头的朝向摆好每一块：各块照 `follow` 跟着转几成；耳朵照 `pose.ear` 绕耳根往外多歪；左右各一个的照 x 镜像再放一个。 */
/**
 * 蹲多深换成头往下沉多少、左右宽多少倍、上下扁多少倍（负的蹲是往上抻：头升起来、变瘦变高）。
 * @param {Look} look
 * @param {Pose} pose
 */
function crouchOf(look, pose) {
  const k = pose.crouch ?? 0;
  const squish = k * (look.crouch_squish ?? 0);
  return { drop: k * (look.crouch_drop ?? 0), sx: 1 + squish / 2, sy: 1 - squish };
}

function placed(look, pose) {
  const out = [];
  const bend = crouchOf(look, pose);
  for (const shape of look.shapes) {
    const body = turn(pose.yaw * (shape.follow ?? 1) + (pose.turn ?? 0), pose.pitch * (shape.follow ?? 1));
    let tilt = shape.tilt;
    // 蹲：鳍不动（脚踩在原地）；头往下沉、压扁，耳朵跟着头顶一起下来
    let center = shape.part === 'fin' ? shape.center : [shape.center[0] * bend.sx, shape.center[1] * bend.sy - bend.drop, shape.center[2]];
    const radii = shape.part === 'head' ? [shape.radii[0] * bend.sx, shape.radii[1] * bend.sy, shape.radii[2]] : shape.radii;
    if (shape.part === 'ear' && pose.ear) {
      tilt = shape.tilt + pose.ear * Math.sign(shape.tilt || 1);
      const reach = shape.radii[1];
      const base = add(center, mul(rotZ(shape.tilt), [0, -reach, 0]));
      center = add(base, mul(rotZ(tilt), [0, reach, 0]));
    }
    // 侧身走路：鳍的位置跟着身子转（一前一后在身子底下），朝向不跟着转（侧面看还是斜着的一片，像两条腿），不然只剩一条缝
    const orient = shape.part === 'fin' ? turn(pose.yaw * (shape.follow ?? 1), pose.pitch * (shape.follow ?? 1)) : body;
    const put = (c, t) => out.push({ part: shape.part, turn: matmul(orient, rotZ(t)), center: mul(body, c), radii });
    // 走路：两片鳍前后迈，往前迈的那片抬起来，往后的踩在地上（左边那片是清单里写的，右边那片是它的镜像）。正面看是一抬一落，
    // 侧过身看是一前一后
    const s = shape.part === 'fin' ? (pose.stride ?? 0) : 0;
    const [lift, swing] = [look.stride_lift ?? 0, look.stride_swing ?? 0];
    put([center[0], center[1] + lift * Math.max(0, s), center[2] + swing * s], tilt);
    if (shape.mirror) put([-center[0], center[1] + lift * Math.max(0, -s), center[2] - swing * s], -tilt);
  }
  return out;
}

/** 一根光线：从 `(x, y)` 朝 −z 打进去，停在最近的面上：打中哪一块、那里的法线、那一点在头自己的坐标里在哪。 */
function shoot(pieces, head, x, y) {
  const origin = [x, y, 10];
  const dir = [0, 0, -1];
  let best = null;
  for (const p of pieces) {
    const o = div(mulT(p.turn, sub(origin, p.center)), p.radii);
    const d = div(mulT(p.turn, dir), p.radii);
    const a = dot(d, d);
    const b = 2 * dot(o, d);
    const c = dot(o, o) - 1;
    const disc = b * b - 4 * a * c;
    if (disc < 0) continue;
    const t = (-b - Math.sqrt(disc)) / (2 * a);
    if (t <= 0 || (best && best.t <= t)) continue;
    const on = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
    best = { t, part: p.part, normal: normalize(mul(p.turn, div(on, p.radii))) };
  }
  return best && { part: best.part, normal: best.normal, point: mulT(head, [x, y, 10 - best.t]) };
}

/**
 * 头上这一点落没落在脸上：只看正面，位置用头朝前时的 x、y。闭着眼时没有眼睛。嘴是那道锯齿线：`open`（0 到 1）张开时上下两排
 * 各往外挪一半，中间是嘴里；肚子上的圈是一盏灯：一圈线，亮着时圈里是灯（2026-09-30 项目主人定：那个圈不是嘴，当作灯，会闪）。
 */
function onFace(face, p, blink, open, lamp) {
  if (p[2] <= 0) return null;
  const q = [p[0], p[1]];
  if (!blink && face.eyes.some(([a, b]) => segment(q, a, b) < face.eye_width)) return 'eye';
  const line = face.line;
  const half = (open * face.mouth_gap) / 2;
  const [x0, x1] = [line[0][0], line[line.length - 1][0]];
  if (half > face.line_width && q[0] > x0 && q[0] < x1) {
    const y = zigzagAt(line, q[0]);
    if (q[1] < y + half && q[1] > y - half) return 'mouth';
  }
  const at = (pt, dy) => [pt[0], pt[1] + dy];
  for (let i = 1; i < line.length; i++) {
    for (const dy of half > 0 ? [half, -half] : [0]) if (segment(q, at(line[i - 1], dy), at(line[i], dy)) < face.line_width) return 'line';
  }
  const m = face.belly;
  const dx = Math.abs(q[0] - m.center[0]) - (m.half[0] - m.round);
  const dy = Math.abs(q[1] - m.center[1]) - (m.half[1] - m.round);
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0)) + Math.min(Math.max(dx, dy), 0);
  if (outside <= m.round && outside >= m.round - m.ring) return 'belly';
  if (lamp && outside < m.round - m.ring) return 'lamp';
  return null;
}

/** 锯齿线在 `x` 那里多高（照几个点连起来的折线）。 */
function zigzagAt(line, x) {
  for (let i = 1; i < line.length; i++) {
    const [a, b] = [line[i - 1], line[i]];
    if (x >= a[0] && x <= b[0]) return a[1] + ((b[1] - a[1]) * (x - a[0])) / (b[0] - a[0] || 1);
  }
  return line[line.length - 1][1];
}

/** 点到线段的距离。 */
function segment(p, a, b) {
  const ab = [b[0] - a[0], b[1] - a[1]];
  const ap = [p[0] - a[0], p[1] - a[1]];
  const len = ab[0] * ab[0] + ab[1] * ab[1];
  const t = len > 0 ? Math.max(0, Math.min(1, (ap[0] * ab[0] + ap[1] * ab[1]) / len)) : 0;
  return Math.hypot(ap[0] - ab[0] * t, ap[1] - ab[1] * t);
}

const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const div = (a, b) => [a[0] / b[0], a[1] / b[1], a[2] / b[2]];
const normalize = (a) => {
  const len = Math.sqrt(dot(a, a));
  return len > 0 ? [a[0] / len, a[1] / len, a[2] / len] : a;
};
/** `m · v` */
const mul = (m, v) => [dot(m[0], v), dot(m[1], v), dot(m[2], v)];
/** `mᵀ · v`：旋转的逆 */
const mulT = (m, v) => [
  m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
  m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
  m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
];
const matmul = (a, b) => [0, 1, 2].map((i) => [0, 1, 2].map((j) => a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j]));
/** 朝 `yaw`、`pitch`（度）的旋转：先左右，再上下 */
const turn = (yaw, pitch) => matmul(rotX((pitch * Math.PI) / 180), rotY((yaw * Math.PI) / 180));
const rotX = (a) => [[1, 0, 0], [0, Math.cos(a), -Math.sin(a)], [0, Math.sin(a), Math.cos(a)]];
const rotY = (a) => [[Math.cos(a), 0, Math.sin(a)], [0, 1, 0], [-Math.sin(a), 0, Math.cos(a)]];
const rotZ = (a) => [[Math.cos(a), -Math.sin(a), 0], [Math.sin(a), Math.cos(a), 0], [0, 0, 1]];
