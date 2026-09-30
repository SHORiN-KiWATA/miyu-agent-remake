//! 运行日志的配置项（`docs/blueprint/config.md`「M8 的配置项」，施工 8-1）：`log.level`，记到哪一级。
//!
//! 这一步只声明，进清单、生成 JSON Schema 和参考文件；读它、照它换级别随 8-2，运行中换随 8-4。设了环境变量
//! `MIYU_LOG` 的，那一次启动照它（`28-运行日志.md` LG2）。运行日志是整个核心的，只能写在系统配置里。

miyu_config::settings! {
    /// 运行日志的配置。
    pub struct LogSettings in "log" {
        /// 记到哪一级，写法同 `MIYU_LOG`（[`crate::level()`]）。
        level: String = "info" {
            kind: option ["error", "warn", "info", "debug", "trace", "off"],
            layers: [System],
            env: "MIYU_LOG",
            applies: now,
            ui: { page: "advanced", group: "log", control: select },
        },
    }
}

#[cfg(test)]
mod tests;
