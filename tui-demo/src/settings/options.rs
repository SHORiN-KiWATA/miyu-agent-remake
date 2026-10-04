//! 本轮配置编辑的选项与标识长度，住在 resources/settings.json，不藏进布局或按键代码。
use serde::Deserialize;
use std::sync::LazyLock;

/// 本轮配置页编辑器的选项，统一由资源提供。
#[derive(Deserialize)]
pub(super) struct Options {
    /// 供应商接口；空项表示自动选择。
    pub drivers: Vec<String>,
    /// 支持输入的可选标签。
    pub inputs: Vec<String>,
    /// 模型池调用方式。
    pub strategies: Vec<String>,
    /// 与核心标识规则一致的最长字节数。
    pub identifier_max: usize,
}

/// 编译进来的选项，只解析一次。
///
/// # Panics
/// 资源 JSON 不完整时 panic；嵌入资源的契约测试在发布前拦下。
pub(super) fn get() -> &'static Options {
    static OPTIONS: LazyLock<Options> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../../resources/settings.json"))
            .expect("编译进来的配置页选项须为完整 JSON，单元测试守着")
    });
    &OPTIONS
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_choices_are_complete_and_valid() {
        let options = super::get();
        assert!(!options.drivers.is_empty());
        assert!(!options.inputs.is_empty());
        assert!(!options.strategies.is_empty());
        assert!(options.identifier_max > 0);
    }
}
