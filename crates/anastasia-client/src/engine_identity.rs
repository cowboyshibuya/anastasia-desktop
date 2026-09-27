//! Stable presentation IDs for engine-owned string session IDs.

use uuid::Uuid;

pub fn gui_session_id(engine_session_id: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("anastasia-engine-session:{engine_session_id}").as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_is_stable_and_distinct() {
        assert_eq!(gui_session_id("ses_one"), gui_session_id("ses_one"));
        assert_ne!(gui_session_id("ses_one"), gui_session_id("ses_two"));
    }
}
