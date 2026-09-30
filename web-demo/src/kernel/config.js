// @ts-check
//! 配置（蓝图 `web/architecture.md`「配置」，照 `14-配置.md` 第二、三、六节）：每个包的设置项写在清单的 `settings` 里，
//! 校验、合层、设置页都从它来。纯函数。
//!
//! - 分层：出厂（清单里的 `default`）→ 发行版（`distro.json`）→ 个人（这个浏览器），上面的盖下面的；每个最终值说得出来源。
//! - 写错不致命：某一层某一项校验不过，这一项用下面一层的，记一条为什么；不认识的项警告、不用、原样留着。

/**
 * @typedef {{type: string, default: any, min?: number, max?: number, integer?: boolean, choices?: string[], of?: string,
 *   name?: Record<string, string>, description?: Record<string, string>, applies?: 'live'|'reload', advanced?: boolean}} Spec 一个设置项
 * @typedef {{name: string, values: Record<string, any>}} Layer 一层：出厂以上的发行版、个人
 * @typedef {{ok: true, value: any}|{ok: false, error: string}} Checked
 */

const ok = (value) => /** @type {Checked} */ ({ ok: true, value });
const no = (error) => /** @type {Checked} */ ({ ok: false, error });

/** 单个值照类型查（`list`、`map` 里的每一个照 `of` 查）。 */
const SCALAR = {
  number: (spec, v) => {
    if (typeof v !== 'number' || !Number.isFinite(v)) return no('要一个数');
    if (spec.integer && !Number.isInteger(v)) return no('要一个整数');
    if (spec.min != null && v < spec.min) return no(`不能小于 ${spec.min}（范围 ${spec.min}–${spec.max ?? '∞'}）`);
    if (spec.max != null && v > spec.max) return no(`不能大于 ${spec.max}（范围 ${spec.min ?? '-∞'}–${spec.max}）`);
    return ok(v);
  },
  duration: (spec, v) => (typeof v === 'number' && v >= 0 && Number.isFinite(v) ? ok(v) : no('要一个不小于 0 的毫秒数')),
  boolean: (spec, v) => (typeof v === 'boolean' ? ok(v) : no('要 true 或 false')),
  choice: (spec, v) => ((spec.choices ?? []).includes(v) ? ok(v) : no(`只能是 ${(spec.choices ?? []).join('、')} 之一`)),
  text: (spec, v) => (typeof v === 'string' ? ok(v) : no('要一段字')),
  color: (spec, v) => (typeof v === 'string' && v.trim() !== '' ? ok(v) : no('要一个颜色（CSS 的写法）')),
  key: (spec, v) => (typeof v === 'string' && v.trim() !== '' ? ok(v) : no('要一个按键')),
  // 一整块数据（词库这类结构复杂的）：是一个 JSON 对象或一串就收下，里面怎么用由包自己查
  json: (spec, v) => (v !== null && typeof v === 'object' ? ok(v) : no('要一块 JSON（对象或一串）')),
};

/**
 * 查一个值合不合这一项。
 * @param {Spec} spec
 * @param {any} value
 * @returns {Checked}
 */
export function check(spec, value) {
  if (spec.type === 'list') {
    if (!Array.isArray(value)) return no('要一串');
    for (const item of value) {
      const r = check({ ...spec, type: spec.of ?? 'text' }, item);
      if (!r.ok) return no(`其中一个不对：${r.error}`);
    }
    return ok(value);
  }
  if (spec.type === 'map') {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return no('要一组「名字: 值」');
    for (const [k, item] of Object.entries(value)) {
      const r = check({ ...spec, type: spec.of ?? 'text' }, item);
      if (!r.ok) return no(`${k} 不对：${r.error}`);
    }
    return ok(value);
  }
  const scalar = SCALAR[spec.type];
  return scalar ? scalar(spec, value) : no(`不认识的类型 ${spec.type}`);
}

/**
 * 合出最终值：出厂值打底，照 `layers` 的先后一层层盖；写错的那一项退回下一层；不认识的项记下、不用。
 * @param {Record<string, Spec>} settings 清单里的设置项
 * @param {Layer[]} layers 出厂以上的几层，下面的在前
 */
export function resolve(settings, layers) {
  /** @type {Record<string, any>} */
  const values = {};
  /** @type {Record<string, string>} */
  const origins = {};
  const errors = [];
  const unknown = [];
  for (const [key, spec] of Object.entries(settings)) {
    values[key] = spec.default;
    origins[key] = 'default';
  }
  for (const layer of layers) {
    for (const [key, value] of Object.entries(layer.values ?? {})) {
      const spec = settings[key];
      if (!spec) {
        unknown.push({ key, layer: layer.name });
        continue;
      }
      const r = check(spec, value);
      if (!r.ok) {
        errors.push({ key, layer: layer.name, error: r.error });
        continue;
      }
      values[key] = r.value;
      origins[key] = layer.name;
    }
  }
  return { values, origins, errors, unknown };
}

/**
 * 清单自己的毛病：出厂值过不了自己的校验、不认识的类型。有的话这个包故障（是它的 bug），别的包照常。
 * @param {Record<string, Spec>} settings
 */
export function problems(settings) {
  const out = [];
  for (const [key, spec] of Object.entries(settings)) {
    const r = check(spec, spec.default);
    if (!r.ok) out.push({ key, error: r.error });
  }
  return out;
}
