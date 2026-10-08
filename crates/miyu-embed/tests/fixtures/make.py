"""生成 miyu-embed 的测试数据（施工 R-5 上）。不进门禁，换数据时手动跑一次：

    python make.py <Xenova/bge-small-zh-v1.5 的文件所在的目录> <这个 fixtures 目录>

那个目录里要有 tokenizer.json 和 model_quantized.onnx（Release models-bge-small-zh-v1.5 里也有后一个）。要装
tokenizers 0.23.2、onnx、onnxruntime、numpy。生成：

1. tokens.json：真词表上，Hugging Face tokenizers 对一批句子的结果（不截断）。字里写着 [CLS] 这种的不放：
   tokenizers 会把它当成那个特殊的词，miyu-embed 照普通的字切（不让人说的话变成控制用的词）。
2. tiny/：手造的小模型、它的词表，和它对几组编号的输出（照 onnxruntime 算）。清单 tiny/manifest.toml 是手写的。
3. real.json：真模型对几句的整个向量，测试照余弦比。
"""
import json, sys, numpy as np, onnx, onnxruntime as ort
from onnx import helper, TensorProto, numpy_helper
from tokenizers import Tokenizer
D, OUT = sys.argv[1], sys.argv[2]
tok = Tokenizer.from_file(D + "/tokenizer.json")
texts = [
    "", "   ", "\t\n\r", "猫", "用户养了一只猫，叫团子，三岁", "回答先说结论，再给依据。",
    "我的显卡是 RTX 4090", "rtx 4090", "Hello World!", "hello world", "unaffable", "naïve café",
    "ペットの猫", "한국어 텍스트", "emoji 😀 和 Ⅻ", "全角！？（）【】“”‘’", "a,b;c:d", "e-mail@example.com",
    "零宽​空格", "BOM﻿在中间", "控制\u0001字符\u007f", "替换�字符", "不间断 空格",
    "x" * 100, "x" * 101, "中" * 5 + "abc" * 40, "###", "##abc", "  前后 空白  ",
    "2026-10-09 12:30", "3.14159", "C++ 和 C#", "𠀀𪚥 扩展区的字", "豈 兼容区", "ｆｕｌｌ ｗｉｄｔｈ",
    "Ⅰ Ⅱ ①②", "á 组合音标", "tab\there", "line1\nline2",
]
json.dump([{"text": t, "ids": tok.encode(t).ids} for t in texts], open(OUT + "/tokens.json", "w"), ensure_ascii=False, indent=0)

# 小模型：词表 21 个词，嵌入 4 维。输出 = 每个位置的嵌入 + 类型的嵌入 + 照掩码加起来的全句的和：第一格（[CLS]）随整句变。
vocab = ["[PAD]", "[UNK]", "[CLS]", "[SEP]", "[MASK]", "猫", "狗", "喝", "茶", "我", "的", "，", "。", "!",
         "un", "##aff", "##able", "hello", "world", "##s", "a"]
open(OUT + "/tiny/vocab.txt", "w").write("\n".join(vocab) + "\n")
rng = np.random.default_rng(7)
E = rng.normal(size=(len(vocab), 4)).astype(np.float32).round(3)
T = rng.normal(size=(2, 4)).astype(np.float32).round(3)
ids = helper.make_tensor_value_info("input_ids", TensorProto.INT64, ["batch", "seq"])
mask = helper.make_tensor_value_info("attention_mask", TensorProto.INT64, ["batch", "seq"])
types = helper.make_tensor_value_info("token_type_ids", TensorProto.INT64, ["batch", "seq"])
out = helper.make_tensor_value_info("last_hidden_state", TensorProto.FLOAT, ["batch", "seq", 4])
nodes = [
    helper.make_node("Gather", ["E", "input_ids"], ["h"]),
    helper.make_node("Gather", ["T", "token_type_ids"], ["t"]),
    helper.make_node("Add", ["h", "t"], ["ht"]),
    helper.make_node("Cast", ["attention_mask"], ["mf"], to=TensorProto.FLOAT),
    helper.make_node("Unsqueeze", ["mf", "axis"], ["m"]),
    helper.make_node("Mul", ["ht", "m"], ["hm"]),
    helper.make_node("ReduceSum", ["hm", "seqaxis"], ["s"], keepdims=1),
    helper.make_node("Add", ["hm", "s"], ["last_hidden_state"]),
]
inits = [numpy_helper.from_array(E, "E"), numpy_helper.from_array(T, "T"),
         numpy_helper.from_array(np.array([-1], dtype=np.int64), "axis"),
         numpy_helper.from_array(np.array([1], dtype=np.int64), "seqaxis")]
g = helper.make_graph(nodes, "tiny", [ids, mask, types], [out], inits)
m = helper.make_model(g, opset_imports=[helper.make_opsetid("", 13)], producer_name="miyu-embed-fixture")
m.ir_version = 8
onnx.checker.check_model(m)
onnx.save(m, OUT + "/tiny/model.onnx")
s = ort.InferenceSession(OUT + "/tiny/model.onnx", providers=["CPUExecutionProvider"])
cases = [[2, 9, 10, 5, 3], [2, 7, 8, 3], [2, 17, 18, 13, 3], [2, 1, 3], [2, 5, 5, 5, 5, 3], [2, 3]]
expected = []
for c in cases:
    a = np.array([c], dtype=np.int64)
    v = s.run(None, {"input_ids": a, "attention_mask": np.ones_like(a), "token_type_ids": np.zeros_like(a)})[0][0, 0]
    expected.append({"ids": c, "vector": [float(x) for x in (v / np.linalg.norm(v)).astype(np.float32)]})
json.dump(expected, open(OUT + "/tiny/expected.json", "w"), indent=0)
print("ok", len(texts))

# 真模型的对照：几句的整个向量（归一化的 [CLS]），照 Python 的 onnxruntime 算，测试照余弦比。
real = ort.InferenceSession(D + "/model_quantized.onnx", providers=["CPUExecutionProvider"])
rows = []
for t in ["用户养了一只猫，叫团子，三岁", "回答先说结论，再给依据", "rtx 4090 显卡", "喝什么茶"]:
    a = np.array([tok.encode(t).ids], dtype=np.int64)
    v = real.run(None, {"input_ids": a, "attention_mask": np.ones_like(a), "token_type_ids": np.zeros_like(a)})[0][0, 0]
    rows.append({"text": t, "vector": [round(float(x), 7) for x in v / np.linalg.norm(v)]})
json.dump(rows, open(OUT + "/real.json", "w"), ensure_ascii=False)
