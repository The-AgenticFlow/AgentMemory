use engram_core::{Episode, Session, SessionMode, WorkingContext};
use uuid::Uuid;

#[test]
fn episode_session_and_context_can_be_created() {
    let session_id = Uuid::new_v4();
    let episode = Episode::new("read paper", "research mode", "stored", session_id);
    let session = Session::new(
        Some(Uuid::new_v4()),
        "find memory architecture",
        SessionMode::Exploration,
        "research task",
    );
    let context = WorkingContext::new(session.id, "task-001");

    assert_eq!(episode.session_id, session_id);
    assert_eq!(session.current_mode, SessionMode::Exploration);
    assert_eq!(context.session_id, session.id);
}
