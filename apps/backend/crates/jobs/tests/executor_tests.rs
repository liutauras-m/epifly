//! Unit tests for the `jobs` crate.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use common::memory::InMemoryAuditStore;
    use jobs::{BackgroundJob, JobContext, JobExecutor, JobRegistry, TaskState};
    use std::sync::Arc;
    use tokio::time::{Duration, sleep};

    fn make_ctx() -> Arc<JobContext> {
        Arc::new(JobContext::new(Arc::new(InMemoryAuditStore::new())))
    }

    // ── Simple echo background job ────────────────────────────────────────────

    struct EchoJob;

    #[async_trait]
    impl BackgroundJob for EchoJob {
        fn name(&self) -> &str {
            "echo"
        }
        async fn run(
            &self,
            input: serde_json::Value,
            _ctx: Arc<JobContext>,
        ) -> anyhow::Result<serde_json::Value> {
            Ok(input)
        }
    }

    struct FailJob;

    #[async_trait]
    impl BackgroundJob for FailJob {
        fn name(&self) -> &str {
            "fail"
        }
        async fn run(
            &self,
            _input: serde_json::Value,
            _ctx: Arc<JobContext>,
        ) -> anyhow::Result<serde_json::Value> {
            anyhow::bail!("intentional test failure")
        }
    }

    fn make_registry() -> Arc<JobRegistry> {
        let ctx = make_ctx();
        let mut reg = JobRegistry::new(ctx);
        reg.register_background(EchoJob);
        reg.register_background(FailJob);
        Arc::new(reg)
    }

    #[tokio::test]
    async fn test_echo_job_completes() {
        let reg = make_registry();
        let exec = JobExecutor::new(reg);
        let input = serde_json::json!({"hello": "world"});
        let task_id = exec
            .enqueue("echo", input.clone(), Some("t1"))
            .await
            .unwrap();

        // Poll until complete
        let mut attempts = 0;
        loop {
            sleep(Duration::from_millis(20)).await;
            let status = exec.get_status(task_id).await.unwrap();
            if status.state == TaskState::Completed {
                assert_eq!(status.result, Some(input));
                break;
            }
            attempts += 1;
            assert!(attempts < 50, "job did not complete in time");
        }
    }

    #[tokio::test]
    async fn test_fail_job_records_error() {
        let reg = make_registry();
        let exec = JobExecutor::new(reg);
        let task_id = exec
            .enqueue("fail", serde_json::Value::Null, None)
            .await
            .unwrap();

        let mut attempts = 0;
        loop {
            sleep(Duration::from_millis(20)).await;
            let status = exec.get_status(task_id).await.unwrap();
            if status.state == TaskState::Failed {
                assert!(
                    status
                        .error
                        .as_deref()
                        .unwrap_or("")
                        .contains("intentional")
                );
                break;
            }
            attempts += 1;
            assert!(attempts < 50, "job did not fail in time");
        }
    }

    #[tokio::test]
    async fn test_unknown_job_returns_error() {
        let reg = make_registry();
        let exec = JobExecutor::new(reg);
        let result = exec
            .enqueue("nonexistent", serde_json::Value::Null, None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let reg = make_registry();
        let exec = JobExecutor::new(reg);
        let id1 = exec
            .enqueue("echo", serde_json::json!(1), None)
            .await
            .unwrap();
        let id2 = exec
            .enqueue("echo", serde_json::json!(2), None)
            .await
            .unwrap();

        // Wait a bit
        sleep(Duration::from_millis(100)).await;

        let tasks = exec.list_tasks(10).await;
        let ids: Vec<_> = tasks.iter().map(|t| t.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn tasks_are_visible_only_to_their_tenant() {
        let reg = make_registry();
        let exec = JobExecutor::new(reg);
        let a = exec
            .enqueue("echo", serde_json::json!("a"), Some("tenant-a"))
            .await
            .unwrap();
        let b = exec
            .enqueue("echo", serde_json::json!("b"), Some("tenant-b"))
            .await
            .unwrap();
        let platform = exec
            .enqueue("echo", serde_json::json!("p"), None)
            .await
            .unwrap();
        sleep(Duration::from_millis(100)).await;

        assert!(exec.get_status_for_tenant(a, "tenant-a").await.is_some());
        assert!(exec.get_status_for_tenant(a, "tenant-b").await.is_none());
        assert!(
            exec.get_status_for_tenant(platform, "tenant-a")
                .await
                .is_none()
        );
        assert!(exec.subscribe_for_tenant(b, "tenant-a").await.is_none());

        let listed: Vec<_> = exec
            .list_tasks_for_tenant("tenant-a", 50)
            .await
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(listed, vec![a]);
        assert_eq!(exec.list_tasks(50).await.len(), 3);
    }
}
