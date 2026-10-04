//! 逐格打光线画吉祥物（蓝图 `tui.md`「空会话的首页」第 5 条）：一格打 2×2 根平行的光线，从看的人那边
//! 垂直打进去，停在最近的面上，照那里多亮从 `ramp` 挑字。打中头的正面时看落没落在脸上：眼睛写 `eye_mark`、
//! 锯齿线（嘴）和肚子上那一圈写 `line_mark`、张开的嘴里是洞。四根里有两根以上落在同一样上才算它。

use super::Pose;
use super::model::{Face, Look, Part};

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

/// 画出来的一格：写哪个字、算哪一块（颜色照它取）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// 字。
    pub mark: char,
    /// 哪一块。
    pub part: Part,
}

/// 一格里四根光线的落点：格子左上角起，以一格为 1。左右、上下都对称，正面画出来才左右对称。
const SAMPLES: [(f64, f64); 4] = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];

/// 摆成 `pose` 时画成的格子：`rows` 行、每行 `cols` 格；没打中的、脸上的洞是 `None`。
pub fn render(look: &Look, pose: &Pose) -> Vec<Vec<Option<Cell>>> {
    let head = turn(pose.yaw, pose.pitch);
    let pieces = placed(look, pose);
    let light = normalize(look.light);
    let ramp: Vec<char> = look.ramp.chars().collect();
    let half = f64::from(look.cols) / 2.0;
    (0..look.rows)
        .map(|r| {
            (0..look.cols)
                .map(|c| {
                    let mut tally = Tally::default();
                    for (dx, dy) in SAMPLES {
                        let x = (f64::from(c) + dx - half) / look.radius;
                        let y =
                            -(f64::from(r) + dy - look.center_row) * look.cell_aspect / look.radius;
                        let Some((part, normal, point)) = shoot(&pieces, &head, x, y) else {
                            continue;
                        };
                        let lum = look.ambient + (1.0 - look.ambient) * dot(normal, light).max(0.0);
                        match (part, face(&look.face, point, pose)) {
                            (Part::Head, Some(mark)) => tally.face[mark as usize] += 1,
                            _ => tally.lit(part, lum),
                        }
                    }
                    tally.cell(look, &ramp)
                })
                .collect()
        })
        .collect()
}

/// 脸上的记号。
#[derive(Clone, Copy)]
enum Mark {
    Eye = 0,
    Line = 1,
    /// 张开的嘴里。
    Hole = 2,
}

/// 一格里四根光线落在哪。
#[derive(Default)]
struct Tally {
    /// 头、耳朵、鳍各有几根打中亮面。
    parts: [u8; 3],
    /// 亮度加起来。
    lum: f64,
    /// 眼睛、线、嘴里各几根。
    face: [u8; 3],
}

impl Tally {
    fn lit(&mut self, part: Part, lum: f64) {
        let at = match part {
            Part::Head | Part::Eye => 0,
            Part::Ear => 1,
            Part::Fin => 2,
        };
        self.parts[at] += 1;
        self.lum += lum;
    }

    /// 定这一格：眼睛、洞、线够两根的照它；别的照亮度挑字，边上打中得少的自然淡一些。
    fn cell(&self, look: &Look, ramp: &[char]) -> Option<Cell> {
        let [eye, line, hole] = self.face;
        if eye >= 2 {
            return Some(Cell {
                mark: look.face.eye_mark,
                part: Part::Eye,
            });
        }
        if hole >= 2 {
            return None;
        }
        if line >= 2 {
            return Some(Cell {
                mark: look.face.line_mark,
                part: Part::Head,
            });
        }
        // 只让没打中的光线把亮度拉低（轮廓自然淡下去）；落在脸上的那几根不算，锯齿线、眼睛边上不起杂字。
        let lit: u8 = self.parts.iter().sum();
        let face: u8 = self.face.iter().sum();
        let missed = SAMPLES.len() as f64 - f64::from(lit + face);
        let top = ramp.len().checked_sub(1)?;
        let level = (self.lum / (f64::from(lit) + missed) * top as f64).round() as usize;
        let mark = *ramp.get(level.min(top)).filter(|_| level > 0)?;
        let most = (0..3).max_by_key(|&i| (self.parts[i], 3 - i)).unwrap_or(0);
        let part = [Part::Head, Part::Ear, Part::Fin][most];
        Some(Cell { mark, part })
    }
}

/// 摆好的一块：自身到世界的旋转、世界里的中心、三个半径。
struct Placed {
    part: Part,
    turn: M3,
    center: V3,
    radii: V3,
}

/// 照头的朝向摆好每一块：各块照 `follow` 跟着转几成；耳朵照 `pose.ear` 往外多歪；左右各一个的照 x 镜像再放一个。
fn placed(look: &Look, pose: &Pose) -> Vec<Placed> {
    let mut out = Vec::new();
    for shape in &look.shapes {
        let [x, y, z] = shape.center;
        let body = turn(pose.yaw * shape.follow, pose.pitch * shape.follow);
        // 耳朵绕耳根往外歪：耳根不动，尖往外甩。
        let (tilt, [x, y, z]) = if shape.part == Part::Ear && pose.ear != 0.0 {
            let tilt = shape.tilt + pose.ear * shape.tilt.signum();
            let reach = shape.radii[1];
            let base = add(shape.center, mul(&rot_z(shape.tilt), [0.0, -reach, 0.0]));
            (tilt, add(base, mul(&rot_z(tilt), [0.0, reach, 0.0])))
        } else {
            (shape.tilt, [x, y, z])
        };
        let mut put = |center: V3, tilt: f64| {
            out.push(Placed {
                part: shape.part,
                turn: matmul(&body, &rot_z(tilt)),
                center: mul(&body, center),
                radii: shape.radii,
            });
        };
        put([x, y, z], tilt);
        if shape.mirror {
            put([-x, y, z], -tilt);
        }
    }
    out
}

/// 朝 `yaw`、`pitch`（度）的旋转：先左右，再上下。
fn turn(yaw: f64, pitch: f64) -> M3 {
    matmul(&rot_x(pitch.to_radians()), &rot_y(yaw.to_radians()))
}

/// 一根光线：从 `(x, y)` 朝 −z 打进去，停在最近的面上。给出打中哪一块、那里的法线（世界里）、
/// 那一点在头自己的坐标里在哪（脸照它画，跟着头转）。
fn shoot(pieces: &[Placed], head: &M3, x: f64, y: f64) -> Option<(Part, V3, V3)> {
    let origin = [x, y, 10.0];
    let dir = [0.0, 0.0, -1.0];
    let mut best: Option<(f64, Part, V3)> = None;
    for p in pieces {
        let o = div(mul_t(&p.turn, sub(origin, p.center)), p.radii);
        let d = div(mul_t(&p.turn, dir), p.radii);
        let (a, b, c) = (dot(d, d), 2.0 * dot(o, d), dot(o, o) - 1.0);
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            continue;
        }
        let t = (-b - disc.sqrt()) / (2.0 * a);
        if t <= 0.0 || best.as_ref().is_some_and(|(near, ..)| *near <= t) {
            continue;
        }
        let on = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
        best = Some((t, p.part, normalize(mul(&p.turn, div(on, p.radii)))));
    }
    best.map(|(t, part, normal)| (part, normal, mul_t(head, [x, y, 10.0 - t])))
}

/// 头上这一点落没落在脸上：只看正面，位置用头朝前时的 x、y。闭着眼时没有眼睛。锯齿线是嘴：张开（`pose.mouth`）时上下
/// 两排各往外挪一半，中间是嘴里；肚子上是一圈线，圈里照身子画（蓝图「空会话的首页」第 5、9 条）。
fn face(face: &Face, p: V3, pose: &Pose) -> Option<Mark> {
    if p[2] <= 0.0 {
        return None;
    }
    let q = [p[0], p[1]];
    if !pose.blink
        && face
            .eyes
            .iter()
            .any(|[a, b]| segment(q, *a, *b) < face.eye_width)
    {
        return Some(Mark::Eye);
    }
    let half = pose.mouth.clamp(0.0, 1.0) * face.mouth_gap / 2.0;
    let line = &face.line;
    let (first, last) = (line.first()?[0], line.last()?[0]);
    if half > face.line_width
        && q[0] > first
        && q[0] < last
        && zigzag(line, q[0]).is_some_and(|y| (q[1] - y).abs() < half)
    {
        return Some(Mark::Hole);
    }
    let shifts: &[f64] = if half > 0.0 { &[half, -half] } else { &[0.0] };
    let on_line = line.windows(2).any(|w| {
        shifts.iter().any(|dy| {
            let (a, b) = ([w[0][0], w[0][1] + dy], [w[1][0], w[1][1] + dy]);
            segment(q, a, b) < face.line_width
        })
    });
    if on_line {
        return Some(Mark::Line);
    }
    let m = &face.belly;
    let dx = (q[0] - m.center[0]).abs() - (m.half[0] - m.round);
    let dy = (q[1] - m.center[1]).abs() - (m.half[1] - m.round);
    let outside = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0);
    if outside <= m.round && outside >= m.round - m.ring {
        return Some(Mark::Line);
    }
    None
}

/// 锯齿线在 `x` 那里多高（照几个点连起来的折线）；在两头外面是 `None`。
fn zigzag(line: &[[f64; 2]], x: f64) -> Option<f64> {
    line.windows(2).find_map(|w| {
        let ([x0, y0], [x1, y1]) = (w[0], w[1]);
        (x >= x0 && x <= x1 && x1 > x0).then(|| y0 + (y1 - y0) * (x - x0) / (x1 - x0))
    })
}

/// 点到线段的距离。
fn segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let len = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (ap[0] - ab[0] * t).hypot(ap[1] - ab[1] * t)
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn div(a: V3, b: V3) -> V3 {
    [a[0] / b[0], a[1] / b[1], a[2] / b[2]]
}

fn normalize(a: V3) -> V3 {
    let len = dot(a, a).sqrt();
    if len > 0.0 {
        [a[0] / len, a[1] / len, a[2] / len]
    } else {
        a
    }
}

/// `m · v`。
fn mul(m: &M3, v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

/// `mᵀ · v`：旋转的逆。
fn mul_t(m: &M3, v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

fn matmul(a: &M3, b: &M3) -> M3 {
    let col = |j: usize| [b[0][j], b[1][j], b[2][j]];
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| dot(a[i], col(j))))
}

/// 绕 x 轴：往下看为正。
fn rot_x(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

/// 绕 y 轴：往右看为正。
fn rot_y(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

/// 绕 z 轴：逆时针为正。
fn rot_z(a: f64) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}
