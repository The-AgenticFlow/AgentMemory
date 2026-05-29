use engram_core::SessionMode;
use engram_runtime::MemorySystem;

#[tokio::test]
async fn pipeline_can_ingest_and_retrieve_a_memory() {
    let system = MemorySystem::new();
    let mut handle = system
        .open_session(
            None,
            "remember research preferences",
            SessionMode::Exploration,
            "research assistant task",
        )
        .await
        .expect("session should open");

    let ingestion = system
        .process_episode(
            &mut handle,
            "read paper on memory consolidation",
            "research assistant task",
            "successfully stored the paper summary",
        )
        .await
        .expect("episode should be processed");

    assert!(ingestion.accepted);
    assert!(matches!(
        ingestion.state,
        engram_core::IngestionState::Accepted
    ));
    assert!(ingestion.engram_id.is_some());

    let retrieval = system
        .retrieve(&handle, "memory consolidation paper")
        .await
        .expect("retrieval should succeed");

    assert_ne!(retrieval.mode, engram_core::RetrievalState::Default);
    assert!(!retrieval.candidates.is_empty());
    assert!(!retrieval.knowledge.facts.is_empty());
    assert!(
        retrieval
            .candidates
            .iter()
            .all(|candidate| candidate.engram.access_count > 0)
    );
}
