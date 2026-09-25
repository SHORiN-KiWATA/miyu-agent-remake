//! 用字符串写的几种取值（`docs/designs/03-事件模型.md` 第三节）。
//! 认识的读成对应的一种；不认识的是新版本才有的，原样留成 `Other`，写出去还是原样。

/// 生成这样一种取值：只列出认识的几个值，和它们在 JSON 里的写法。
macro_rules! text_enum {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident = $text:literal,)+ }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum $name {
            $($(#[$vdoc])* $variant,)+
            /// 不认识的取值，原样留着。
            Other(String),
        }

        impl $name {
            /// 在 JSON 里的写法。
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $text,)+
                    Self::Other(text) => text,
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = <String as serde::Deserialize>::deserialize(d)?;
                Ok(match text.as_str() {
                    $($text => Self::$variant,)+
                    _ => Self::Other(text),
                })
            }
        }
    };
}

pub(crate) use text_enum;

#[cfg(test)]
mod tests;
