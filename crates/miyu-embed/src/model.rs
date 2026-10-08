//! 照清单载入一份模型、算一句的向量（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 4 款）：ONNX Runtime 静态链接在
//! 这个小程序里，主程序不带。一次一条、单线程：旧版实测一次 32 条、16 线程时内存冲到 2 GB 不还。

use std::fmt;
use std::path::{Path, PathBuf};

use ort::session::Session;
use ort::value::Tensor;

use crate::manifest::{Manifest, Pooling, Role};
use crate::wordpiece::Tokenizer;

/// 一份载入了的模型。
#[derive(Debug)]
pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
    manifest: Manifest,
}

/// 载入不了。
#[derive(Debug)]
pub enum LoadError {
    /// 清单写的文件在目录里没有。
    Missing(PathBuf),
    /// 词表读不了、不合写法。
    Vocab {
        /// 哪个文件。
        path: PathBuf,
        /// 为什么。
        error: String,
    },
    /// ONNX Runtime 载入不了模型。
    Model {
        /// 哪个文件。
        path: PathBuf,
        /// ONNX Runtime 说的原因。
        error: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Missing(path) => write!(f, "cannot find {}", path.display()),
            LoadError::Vocab { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            LoadError::Model { path, error } => {
                write!(f, "cannot load {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// 一句算不出来。
#[derive(Debug, Clone, PartialEq)]
pub enum EmbedError {
    /// ONNX Runtime 跑不了：输入的名字、类型对不上，或者跑的时候出错。
    Run(String),
    /// 模型交回来的最后一维和清单的 `dims` 对不上：清单写错了。
    Dims {
        /// 模型交回来的。
        got: i64,
        /// 清单写的。
        want: usize,
    },
    /// 取出来的那一格全是零、或者有不是数的，归一化不了。
    Degenerate,
}

impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbedError::Run(error) => write!(f, "cannot run the model: {error}"),
            EmbedError::Dims { got, want } => {
                write!(
                    f,
                    "the model gives {got} dimensions, the manifest says {want}"
                )
            }
            EmbedError::Degenerate => write!(f, "the vector is zero or not a number"),
        }
    }
}

impl std::error::Error for EmbedError {}

impl Embedder {
    /// 照清单 `manifest` 载入目录 `dir` 里的模型和词表。
    ///
    /// # Errors
    ///
    /// 文件没有，词表读不了、不合写法，ONNX Runtime 载入不了模型。
    pub fn load(manifest: Manifest, dir: &Path) -> Result<Embedder, LoadError> {
        let model = dir.join(&manifest.file(Role::Model).name);
        let vocab = dir.join(&manifest.file(Role::Vocab).name);
        for path in [&model, &vocab] {
            if !path.is_file() {
                return Err(LoadError::Missing(path.clone()));
            }
        }
        let bad_vocab = |error: String| LoadError::Vocab {
            path: vocab.clone(),
            error,
        };
        let text = std::fs::read_to_string(&vocab).map_err(|error| bad_vocab(error.to_string()))?;
        let tokenizer = Tokenizer::new(&text).map_err(|error| bad_vocab(error.to_string()))?;
        // 每一步的错各是一种类型（带着造到一半的那一个），都照原话交上去。
        let failed = |error: &dyn fmt::Display| LoadError::Model {
            path: model.clone(),
            error: error.to_string(),
        };
        let session = Session::builder()
            .map_err(|error| failed(&error))?
            .with_intra_threads(1)
            .map_err(|error| failed(&error))?
            .with_inter_threads(1)
            .map_err(|error| failed(&error))?
            .commit_from_file(&model)
            .map_err(|error| failed(&error))?;
        Ok(Embedder {
            session,
            tokenizer,
            manifest,
        })
    }

    /// 向量记的模型编号（`local:<id>`）。
    pub fn model(&self) -> String {
        self.manifest.model()
    }

    /// 向量几维。
    pub fn dims(&self) -> usize {
        self.manifest.dims
    }

    /// 算 `text` 的向量：照清单切词、截断，跑一次模型，取 `[CLS]` 那一格，归一化。
    ///
    /// # Errors
    ///
    /// ONNX Runtime 跑不了；模型交回来的维数和清单对不上；取出来的全是零、或者有不是数的。
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let ids = self.tokenizer.encode(text, self.manifest.max_tokens);
        let shape = [1, ids.len()];
        let run = |error: ort::Error| EmbedError::Run(error.to_string());
        let mask = Tensor::from_array((shape, vec![1_i64; ids.len()])).map_err(run)?;
        let types = Tensor::from_array((shape, vec![0_i64; ids.len()])).map_err(run)?;
        let ids = Tensor::from_array((shape, ids)).map_err(run)?;
        let outputs = self
            .session
            .run(ort::inputs!["input_ids" => ids, "attention_mask" => mask, "token_type_ids" => types])
            .map_err(run)?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(run)?;
        let want = self.manifest.dims;
        let got = shape.last().copied().unwrap_or(0);
        if usize::try_from(got).ok() != Some(want) {
            return Err(EmbedError::Dims { got, want });
        }
        let vector = match self.manifest.pooling {
            Pooling::Cls => values.get(..want),
        };
        normalized(vector.ok_or(EmbedError::Dims { got, want })?)
    }
}

/// `vector` 除以它的长度：以后照点积算相似度（`recall.md` 第三条第 2 款）。
fn normalized(vector: &[f32]) -> Result<Vec<f32>, EmbedError> {
    let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if length == 0.0 || !length.is_finite() {
        return Err(EmbedError::Degenerate);
    }
    Ok(vector.iter().map(|x| x / length).collect())
}
