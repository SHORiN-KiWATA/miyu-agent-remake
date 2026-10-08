// @ts-check
//! 有哪些人格、预设，默认是哪个（软件包 `setup`）：照核心的 `persona.list`、`preset.list`、`config.get` 读，进空会话、打开设置时读一次；
//! 预设的细节（`preset.get`：预设写的默认人格、开关）用到才读、记着。读不到的（旧核心）那一样是 `null`，界面就不出那个按钮。

export class Catalog {
  /** @param {{request: (method: string, params: any) => Promise<any>}} core */
  constructor(core) {
    this.core = core;
    /** @type {import('./model.js').Persona[]|null} */
    this.personas = null;
    /** @type {import('./model.js').Preset[]|null} */
    this.presets = null;
    /** 配置项 `persona.default`、`preset.default` 的最终值 */
    this.personaDefault = /** @type {string|null} */ (null);
    this.presetDefault = /** @type {string|null} */ (null);
    /** 预设 → `preset.get` 的回应 @type {Map<string, any>} */
    this.details = new Map();
  }

  /** 重新读一遍（预设的细节也作废）。 */
  async load() {
    const [personas, presets, got] = await Promise.all([
      this.core.request('persona.list', {}).then((r) => r?.personas ?? [], () => null),
      this.core.request('preset.list', {}).then((r) => r?.presets ?? [], () => null),
      this.core.request('config.get', { keys: ['persona.default', 'preset.default'] }).catch(() => null),
    ]);
    this.personas = personas;
    this.presets = presets;
    this.personaDefault = got?.items?.['persona.default']?.value ?? null;
    this.presetDefault = got?.items?.['preset.default']?.value ?? null;
    this.details.clear();
  }

  /** 一个预设的细节，读过的直接交；读不到的是 `null`。 @param {string} id */
  async preset(id) {
    if (!this.details.has(id)) this.details.set(id, await this.core.request('preset.get', { preset: id }).catch(() => null));
    return this.details.get(id);
  }

  /** 读过的预设细节（没读过的是 `undefined`）。 @param {string} id */
  known(id) {
    return this.details.get(id);
  }
}
