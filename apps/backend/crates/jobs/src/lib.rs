//! `jobs` — scheduled and background job infrastructure for the ConusAI platform.
//!
//! Build a registry, start the scheduler, and enqueue background tasks:

pub mod admin;
pub mod context;
pub mod executor;
pub mod job;
pub mod jobs;
pub mod registry;
pub mod scheduler;

pub use admin::{JobAdmin, JobKind, JobSummary};
pub use context::JobContext;
pub use executor::{JobExecutor, TaskEvent};
pub use job::{BackgroundJob, ScheduledJob, TaskState, TaskStatus};
pub use registry::JobRegistry;
pub use scheduler::JobSchedulerService;
