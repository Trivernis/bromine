use crate::events::generate_event_id;
use std::collections::HashSet;

#[test]
fn event_ids_work() {
    let mut ids = HashSet::new();

    // simple collision test
    for _ in 0..100000 {
        assert!(ids.insert(generate_event_id()))
    }
}
