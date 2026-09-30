//! JSON Schema 的样子：只有能放进这一层的项，键照段分成一层层的表，格照字母先后，两格缩进，最后一个换行；缺字报
//! 缺了哪一句。和出厂的清单、资源生成的样本逐字节比，在 `miyu-core/tests/settings.rs`：这里拿不到上层的清单。

use crate::item::Layer;
use crate::schema::render;
use crate::test_support::{item, system_only, words};
use crate::words::Missing;

const SYSTEM: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "properties": {
    "a": {
      "properties": {
        "b": {
          "properties": {
            "c": {
              "default": "x",
              "description": "a.b.c 的说明。能写：x 或 y。只能写在系统配置或个人设置里。当场生效。",
              "enum": [
                "x",
                "y"
              ],
              "title": "a.b.c 的名字",
              "type": "string"
            }
          },
          "type": "object"
        },
        "d": {
          "default": "on",
          "description": "a.d 的说明。能写：on 或 off。只能写在系统配置里。当场生效。",
          "enum": [
            "on",
            "off"
          ],
          "title": "a.d 的名字",
          "type": "string"
        }
      },
      "type": "object"
    }
  },
  "type": "object"
}
"#;

const PERSONAL: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "properties": {
    "a": {
      "properties": {
        "b": {
          "properties": {
            "c": {
              "default": "x",
              "description": "a.b.c 的说明。能写：x 或 y。只能写在系统配置或个人设置里。当场生效。",
              "enum": [
                "x",
                "y"
              ],
              "title": "a.b.c 的名字",
              "type": "string"
            }
          },
          "type": "object"
        }
      },
      "type": "object"
    }
  },
  "type": "object"
}
"#;

#[test]
fn each_layer_gets_its_own_items_as_nested_tables() {
    let items = [
        system_only(item("a.d", &["on", "off"], "on")),
        item("a.b.c", &["x", "y"], "x"),
    ];
    let words = words(&items);
    assert_eq!(render(&items, Layer::System, &words).as_deref(), Ok(SYSTEM));
    assert_eq!(
        render(&items, Layer::Personal, &words).as_deref(),
        Ok(PERSONAL)
    );
}

#[test]
fn a_layer_with_nothing_is_an_empty_object() {
    let items = [system_only(item("a.d", &["on", "off"], "on"))];
    assert_eq!(
        render(&items, Layer::Personal, &words(&items)).as_deref(),
        Ok(
            "{\n  \"$schema\": \"http://json-schema.org/draft-07/schema#\",\n  \"properties\": {},\n  \"type\": \"object\"\n}\n"
        )
    );
}

#[test]
fn missing_words_are_named() {
    let items = [item("a.d", &["on", "off"], "on")];
    let mut without_item = words(&items);
    without_item.items.clear();
    assert_eq!(
        render(&items, Layer::System, &without_item),
        Err(Missing("config.items.a.d".to_string()))
    );
    let mut without_sentence = words(&items);
    without_sentence
        .sentences
        .remove("config/schema-description");
    assert_eq!(
        render(&items, Layer::System, &without_sentence),
        Err(Missing("config/schema-description".to_string()))
    );
}
