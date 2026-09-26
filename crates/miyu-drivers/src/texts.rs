//! 驱动写给模型的几句占位（`resources/core/drivers/`，`docs/designs/26-提示词.md` 第八节）：英文
//! 短句，只说发生了什么（26 J3）。
//!
//! - 图片发不了：模型不能看图，没有字段；
//! - 文件发不了：模型不能读这种文件，字段是 `name`、`media_type`；
//! - 工具一个字都没回，没有字段；
//! - 工具结果里的图片、文件挪到了后面：后面那条消息开头的一句，和只有图片、文件，没有字的那条
//!   结果里写的一句，都没有字段。
//!
//! 字段照模板的规矩转义（`08-上下文投影.md` 第五节「模板与转义怎么写」）。由执行器从资源目录读好
//! 交进来，造会话时读一次，冻结在会话上。

use std::collections::BTreeMap;

use miyu_kernel::template::{Template, TemplateError};

/// 那几句，各是一份读好的模板。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverTexts {
    /// 图片发不了。
    image_omitted: Template,
    /// 文件发不了。
    file_omitted: Template,
    /// 工具一个字都没回。
    no_output: Template,
    /// 挪到后面的图片、文件前面的那一句。
    tool_attachments: Template,
    /// 只有图片、文件，没有字的那条结果里写的。
    tool_attachments_only: Template,
}

/// 那几句的原文，照资源目录里的文件名。
#[derive(Debug, Clone, Copy)]
pub struct DriverTextSources<'a> {
    /// `image-omitted.txt`。
    pub image_omitted: &'a str,
    /// `file-omitted.txt`。
    pub file_omitted: &'a str,
    /// `no-output.txt`。
    pub no_output: &'a str,
    /// `tool-attachments.txt`。
    pub tool_attachments: &'a str,
    /// `tool-attachments-only.txt`。
    pub tool_attachments_only: &'a str,
}

impl DriverTexts {
    /// 读几份模板，读好以后拿字段试着换一次。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了不该有的字段，返回 [`TemplateError`]。
    pub fn new(sources: DriverTextSources<'_>) -> Result<DriverTexts, TemplateError> {
        let texts = DriverTexts {
            image_omitted: Template::parse(sources.image_omitted)?,
            file_omitted: Template::parse(sources.file_omitted)?,
            no_output: Template::parse(sources.no_output)?,
            tool_attachments: Template::parse(sources.tool_attachments)?,
            tool_attachments_only: Template::parse(sources.tool_attachments_only)?,
        };
        texts.file_omitted.render(&file("", ""))?;
        for plain in [
            &texts.image_omitted,
            &texts.no_output,
            &texts.tool_attachments,
            &texts.tool_attachments_only,
        ] {
            plain.render(&BTreeMap::new())?;
        }
        Ok(texts)
    }

    /// 模型不能看图，图片换成的这一句。
    pub fn image_omitted(&self) -> String {
        render(&self.image_omitted, &BTreeMap::new())
    }

    /// 模型不能读这个文件，换成的这一句。
    pub fn file_omitted(&self, name: &str, media_type: &str) -> String {
        render(&self.file_omitted, &file(name, media_type))
    }

    /// 工具一个字都没回。
    pub fn no_output(&self) -> String {
        render(&self.no_output, &BTreeMap::new())
    }

    /// 挪到后面的图片、文件前面的那一句。
    pub fn tool_attachments(&self) -> String {
        render(&self.tool_attachments, &BTreeMap::new())
    }

    /// 只有图片、文件，没有字的那条结果里写的。
    pub fn tool_attachments_only(&self) -> String {
        render(&self.tool_attachments_only, &BTreeMap::new())
    }
}

/// 造的时候试换过，字段都有，换不出来就是模板的检查漏了。
fn render(template: &Template, fields: &BTreeMap<&str, &str>) -> String {
    template.render(fields).expect("造的时候试换过，字段都有")
}

fn file<'a>(name: &'a str, media_type: &'a str) -> BTreeMap<&'a str, &'a str> {
    BTreeMap::from([("name", name), ("media_type", media_type)])
}

#[cfg(test)]
mod tests;
