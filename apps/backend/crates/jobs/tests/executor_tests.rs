//! Unit tests for the `jobs` crate.

#[cfg(test)]
mod tests {
    use jobs::{
        BackgroundJob, JobContext, JobExecutor, JobRegistry, TaskState,
    };
    use common::memory::InMemoryAuditStore;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    fn make_ctx() -> Arc<JobContext> {
        Arc::new(JobContext::new(
            Arc::new(InMemoryAuditStore::new()),
            "http://localhost:6333",
            None,
            None,
        ))
    }

    // ── Simple echo background job ────────────────────────────────────────────

    struct EchoJob;

    #[async_trait]
    impl BackgroundJob for EchoJob {
        fn name(&self) -> &str { "echo" }
        async fn run(&self, input: serde_json::Value, _ctx: Arc<JobContext>) -> anyhow::Result<serde_json::Value> {
            Ok(input)
        }
    }

    struct FailJob;

    #[async_trait]
    impl BackgroundJob for FailJob {
        fn name(&self) -> &str { "fail" }
        async fn run(&self, _input: serde_json::Value, _ctx: Arc<JobContext>) -> anyhow::Result<serde_json::Value> {
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
        let task_id = exec.enqueue("echo", input.clone()).await.unwrap();

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
        let task_id = exec.enqueue("fail", serde_json::Value::Null).await.unwrap();

        let mut attempts = 0;
        loop {
            sleep(Duration::from_millis(20)).await;
            let status = exec.get_status(task_id).await.unwrap();
            if status.state == TaskState::Failed {
                assert!(status.error.as_deref().unwrap_or("").contains("intentional"));
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
        let result = exec.enqueue("nonexistent", serde_json::Value::Null).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let reg = make_registry();
        let exec = JobExecutor::new(reg);
        let id1 = exec.enqueue("echo", serde_json::json!(1)).await.unwrap();
        let id2 = exec.enqueue("echo", serde_json::json!(2)).await.unwrap();

        // Wait a bit
        sleep(Duration::from_millis(100)).await;

        let tasks = exec.list_tasks(10).await;
        let ids: Vec<_> = tasks.iter().map(|t| t.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }
}
