//! 目录架子（施工 O-2 中，`docs/blueprint/providers.md`「目录换代」）：核心现在的工具目录，提供者登记一次换一代。核心、会话的
//! 执行器、权限策略、回合开头换快照的那一段拿着同一个架子：执行、判权限照现在的那一份找工具，换快照的照代数看换没换。

use std::fmt;
use std::sync::{Arc, PoisonError, RwLock};

use crate::{Catalog, Tool};

/// 目录架子：克隆的是同一个架子。
#[derive(Clone, Default)]
pub struct Shelf {
    inner: Arc<RwLock<Edition>>,
}

/// 架子上的一代：第几代、那一份目录。
#[derive(Clone, Debug, Default)]
pub struct Edition {
    /// 第几代：造架子时是 0，换一次加一。
    pub generation: u64,
    /// 这一代的目录。
    pub catalog: Catalog,
}

impl Shelf {
    /// 架子上放着 `catalog`，第 0 代。
    #[must_use]
    pub fn new(catalog: Catalog) -> Shelf {
        Shelf {
            inner: Arc::new(RwLock::new(Edition {
                generation: 0,
                catalog,
            })),
        }
    }

    /// 现在这一代。
    #[must_use]
    pub fn edition(&self) -> Edition {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 现在的目录。
    #[must_use]
    pub fn current(&self) -> Catalog {
        self.edition().catalog
    }

    /// 现在的目录里叫 `name` 的那一件（[`Catalog::get`]）。
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .catalog
            .get(name)
            .cloned()
    }

    /// 照现在的目录换一份：`change` 交回新的就换上、算一代，交回第几代；交回错误的不动。两处同时换的一个接一个，
    /// 后换的照先换的结果改。
    ///
    /// # Errors
    ///
    /// `change` 交回的错误。
    pub fn replace<E>(
        &self,
        change: impl FnOnce(&Catalog) -> Result<Catalog, E>,
    ) -> Result<u64, E> {
        let mut edition = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        edition.catalog = change(&edition.catalog)?;
        edition.generation += 1;
        Ok(edition.generation)
    }
}

impl fmt::Debug for Shelf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.edition().fmt(f)
    }
}

#[cfg(test)]
mod tests;
