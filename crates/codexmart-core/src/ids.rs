/// New primary key. UUIDv7 keys are unique across devices and sort by
/// creation time, which keeps future multi-device synchronisation simple.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}
