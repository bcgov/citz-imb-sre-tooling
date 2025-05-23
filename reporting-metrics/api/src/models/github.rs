use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentFrequency {
    pub last_30_days: u32,
    pub deployments_per_week: f32,
    pub last_deployment: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadTimeForChanges {
    pub average_lead_time_hours: f32,
    pub median_lead_time_hours: f32,
    pub ninety_percentile_hours: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub name: String,
    pub status: String,
    pub conclusion: String,
    pub created_at: DateTime<Utc>,
    pub duration_minutes: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetrics {
    pub recent_runs: Vec<WorkflowRun>,
    pub average_duration_minutes: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentTag {
    pub environment: String,
    pub tag: String,
    pub deployed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetrics {
    pub deployment_frequency: DeploymentFrequency,
    pub lead_time: LeadTimeForChanges,
    pub workflow_metrics: WorkflowMetrics,
    pub deployment_tags: Vec<DeploymentTag>,
}
