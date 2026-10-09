//! 两路合并（施工 R-5 下）：只有一路的照它的先后；两路都有的排在只有一路的前面；只有向量一路找到的过下限，关键词一路也找到
//! 的不看下限；向量一路的名次照它原来的先后数；分一样的照关键词一路的先后、再照向量一路的。

use super::*;

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|key| (*key).to_string()).collect()
}

fn near(list: &[(&str, f32)]) -> Vec<(String, f32)> {
    list.iter()
        .map(|(key, similar)| ((*key).to_string(), *similar))
        .collect()
}

#[test]
fn one_way_keeps_its_order() {
    assert_eq!(fuse(&keys(&["a", "b", "c"]), &[]), keys(&["a", "b", "c"]));
    assert_eq!(
        fuse(&[], &near(&[("x", 0.9), ("y", 0.5)])),
        keys(&["x", "y"])
    );
    assert!(fuse::<String>(&[], &[]).is_empty());
}

#[test]
fn found_both_ways_comes_first() {
    // c 两路都有：1/63 + 0.5/61 比 a 的 1/61 大。
    let fused = fuse(&keys(&["a", "b", "c"]), &near(&[("c", 0.7), ("x", 0.9)]));
    assert_eq!(fused, keys(&["c", "a", "b", "x"]));
}

#[test]
fn only_by_vector_needs_the_floor() {
    let fused = fuse(
        &keys(&["a"]),
        &near(&[("x", 0.45), ("y", FLOOR - 0.01), ("a", 0.1)]),
    );
    assert_eq!(
        fused,
        keys(&["a", "x"]),
        "y 不到下限；a 关键词也找到了，不看下限"
    );
    // 关键词也找到的，向量那一路的分照样加上（不看下限）：b 靠向量那一点分越过 a。
    let fused = fuse(&keys(&["a", "b"]), &near(&[("b", 0.1)]));
    assert_eq!(fused, keys(&["b", "a"]));
    // y 不算名次以外的：z 的名次照原来的先后数（第 3 个）。
    let fused = fuse(&[], &near(&[("x", 0.9), ("y", 0.1), ("z", 0.8)]));
    assert_eq!(fused, keys(&["x", "z"]));
}

#[test]
fn ties_follow_the_keyword_order_then_the_vector_order() {
    // 关键词一路第 1 名（1/61）和向量一路两个都是第 1 名…… 构造同分：关键词第 2 名的 b（1/62）、向量里只有一路的 x、y
    // 名次 1、2（0.5/61、0.5/62），分各不相同；同分只在同一路同名次时出现，照先来后到。
    let fused = fuse(&keys(&["a", "b"]), &near(&[("x", 0.9), ("y", 0.9)]));
    assert_eq!(fused, keys(&["a", "b", "x", "y"]));
}
