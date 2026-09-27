use super::*;
use crate::event::Body;
use crate::test_support::read_body;

#[test]
fn every_field_reads_back_as_written() {
    let body = r#"{"files":[{"result":57,"effect":0,"path":"/w/a.txt","action":"write","outcome":"changed","found":"sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"},{"result":61,"effect":1,"path":"/w/b","action":"trash","outcome":"restored","trash":"/t/files/b"},{"result":61,"effect":2,"path":"/w/c.txt","action":"untrash","outcome":"restored","hash":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"},{"result":62,"effect":0,"path":"/w/d.txt","action":"write","outcome":"failed","error":"Permission denied (os error 13)"}]}"#;
    match read_body("files.restored", body) {
        Body::FilesRestored(restored) => {
            assert_eq!(restored.files.len(), 4);
            assert_eq!(restored.files[0].result.get(), 57);
            assert_eq!(restored.files[0].outcome, RestoreOutcome::Changed);
            assert!(restored.files[0].found.is_some());
            assert_eq!(restored.files[1].action, RestoreAction::Trash);
            assert_eq!(restored.files[1].trash.as_deref(), Some("/t/files/b"));
            assert_eq!(restored.files[2].action, RestoreAction::Untrash);
            assert!(restored.files[2].hash.is_some());
            assert_eq!(
                restored.files[3].error.as_deref(),
                Some("Permission denied (os error 13)")
            );
            assert_eq!(
                serde_json::to_string(&restored).expect("写得出去"),
                body,
                "写出去一字不差"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_new_action_or_outcome_is_kept_as_it_is() {
    let body = r#"{"files":[{"result":3,"effect":0,"path":"/w/a","action":"rename","outcome":"skipped"}]}"#;
    match read_body("files.restored", body) {
        Body::FilesRestored(restored) => {
            assert_eq!(
                restored.files[0].action,
                RestoreAction::Other("rename".into())
            );
            assert_eq!(
                restored.files[0].outcome,
                RestoreOutcome::Other("skipped".into())
            );
            assert_eq!(serde_json::to_string(&restored).expect("写得出去"), body);
        }
        other => panic!("{other:?}"),
    }
}
