//! 测试共用的几样：造文件、造场所、读一组断定没有问题。字都写在测试里，不碰磁盘。

use std::borrow::Cow;

use miyu_config::Value;

use super::{File, Problem, Rules, Source, Venue, VenueKind};

/// 一份出厂的规则文件。
pub(crate) fn factory(name: &str, text: &str) -> File {
    File {
        source: Source::Factory,
        name: name.to_string(),
        text: text.to_string(),
    }
}

/// 一份系统的规则文件。
pub(crate) fn system(name: &str, text: &str) -> File {
    File {
        source: Source::System,
        ..factory(name, text)
    }
}

/// `qq` 上的一个群，群号是 `number`，断定合写法。
pub(crate) fn group(number: &str) -> Venue {
    Venue::new("qq", VenueKind::Group, number).expect(number)
}

/// `qq` 上的一个私聊，对方的号是 `number`，断定合写法。
pub(crate) fn private(number: &str) -> Venue {
    Venue::new("qq", VenueKind::Private, number).expect(number)
}

/// 一个字的值。
pub(crate) fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

/// 读一组文件，断定没有问题。
pub(crate) fn rules(files: &[File]) -> Rules {
    let read = Rules::parse(files);
    assert_eq!(read.problems, Vec::<Problem>::new());
    read.rules
}

/// 一组文件套到 `venue` 上，某一项的值；没有设到的是空的。
pub(crate) fn value(files: &[File], venue: &Venue, key: &str) -> Option<Value> {
    rules(files)
        .resolve(venue)
        .entries
        .get(key)
        .map(|entry| entry.value.clone())
}
