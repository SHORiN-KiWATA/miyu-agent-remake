//! 本轮配置编辑的选项与标识长度，住在 resources/settings.json，不藏进布局或按键代码。
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
pub(super) struct Options {
    pub drivers: Vec<String>,
    pub inputs: Vec<String>,
    pub strategies: Vec<String>,
    pub identifier_max: usize,
}

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
