//! Tenant-isolation tests for `ThreadStore::list`.
//!
//! Backs the `GET /v1/threads` route added in PR 3.A.6 — the route delegates
//! straight to `state.thread_store.list(&tenant_id, ...)`, so the store-level
//! invariant (tenant A's threads never appear in tenant B's list) is the
//! security contract.

use common::memory::InMemoryThreadStore;
use common::memory::store::ThreadStore;

#[tokio::test]
async fn list_returns_only_threads_for_caller_tenant() {
    let store = InMemoryThreadStore::new();

    // Tenant A creates two threads.
    let a1 = store.create("tenant-a", vec![]).await.unwrap();
    let a2 = store.create("tenant-a", vec![]).await.unwrap();

    // Tenant B creates one thread.
    let b1 = store.create("tenant-b", vec![]).await.unwrap();

    // A's listing must contain a1, a2 and exclude b1.
    let a_list = store.list("tenant-a", 50, None).await.unwrap();
    let a_ids: Vec<String> = a_list.iter().map(|t| t.id.to_string()).collect();
    assert!(a_ids.contains(&a1.id.to_string()));
    assert!(a_ids.contains(&a2.id.to_string()));
    assert!(
        !a_ids.contains(&b1.id.to_string()),
        "tenant-a list leaked tenant-b's thread {}",
        b1.id
    );

    // B's listing must contain b1 and exclude a1, a2.
    let b_list = store.list("tenant-b", 50, None).await.unwrap();
    let b_ids: Vec<String> = b_list.iter().map(|t| t.id.to_string()).collect();
    assert!(b_ids.contains(&b1.id.to_string()));
    assert!(
        !b_ids.contains(&a1.id.to_string()) && !b_ids.contains(&a2.id.to_string()),
        "tenant-b list leaked tenant-a's threads"
    );
}

#[tokio::test]
async fn list_respects_limit_clamp() {
    let store = InMemoryThreadStore::new();
    for _ in 0..5 {
        store.create("tenant-a", vec![]).await.unwrap();
    }

    let two = store.list("tenant-a", 2, None).await.unwrap();
    assert_eq!(two.len(), 2);

    let twenty = store.list("tenant-a", 20, None).await.unwrap();
    assert_eq!(twenty.len(), 5, "limit is a cap, not a quota");
}

#[tokio::test]
async fn empty_listing_for_unknown_tenant() {
    let store = InMemoryThreadStore::new();
    store.create("tenant-a", vec![]).await.unwrap();
    let other = store.list("tenant-unknown", 50, None).await.unwrap();
    assert!(other.is_empty());
}
