use miyu_drivers::Inputs;
use miyu_kernel::id::ModelName;

use super::*;

fn call(max_output: Option<u32>) -> Call {
    Call {
        model: ModelName::parse("m").expect("合模型名的写法"),
        max_output,
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    }
}

/// 快满了的主请求（施工 6-11 三补）：输出上限压到请求给的；原来更小的、请求没给的照原样；原来没写的照它写。
#[test]
fn the_output_is_capped_only_when_the_request_says_so() {
    assert_eq!(capped(&call(Some(8192)), None), None, "平常的照原样");
    assert_eq!(
        capped(&call(Some(8192)), Some(9000)),
        None,
        "原来更小的照原样"
    );
    assert_eq!(capped(&call(Some(8192)), Some(8192)), None);
    assert_eq!(
        capped(&call(Some(8192)), Some(5000)).and_then(|call| call.max_output),
        Some(5000)
    );
    assert_eq!(
        capped(&call(None), Some(5000)).and_then(|call| call.max_output),
        Some(5000),
        "原来没写的照它写：供应商默认的输出可能超窗口"
    );
}
