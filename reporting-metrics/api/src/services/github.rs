use crate::models::github::{
    DeploymentFrequency, LeadTimeForChanges, WorkflowRun, WorkflowMetrics,
    DeploymentTag, RepositoryMetrics,
};
use crate::state::AppState;
use actix_web::web;
use chrono::{DateTime, Utc, Duration};
use log::{error, info};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const GITHUB_API_BASE_URL: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct GitHubWorkflowRun {
    id: u64,
    name: String,
    status: String,
    conclusion: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GitHubDeployment {
    id: u64,
    sha: String,
    ref_field: String,
    created_at: DateTime<Utc>,
    #[serde(rename = "payload")]
    payload_json: serde_json::Value,
}

pub async fn collect_github_metrics(app_state: web::Data<AppState>) {
    info!("Starting GitHub metrics collection");

    loop {
        let services = {
            let services_lock = app_state.services.lock().unwrap();
            services_lock.clone()
        };

        for service in services.iter() {
            if let Some(repo_url) = &service.github_repo {
                match fetch_repository_metrics(repo_url, &app_state).await {
                    Ok(metrics) => {
                        let mut metrics_cache = app_state.metrics_cache.lock().unwrap();
                        let service_metrics = metrics_cache
                            .entry(service.name.clone())
                            .or_insert_with(HashMap::new);

                        service_metrics.insert("github_metrics".to_string(), serde_json::to_value(metrics).unwrap());

                        info!("Updated GitHub metrics for service: {}", service.name);
                    }
                    Err(e) => {
                        error!("Failed to collect GitHub metrics for {}: {}", service.name, e);
                    }
                }
            }
        }

        // Sleep for 15 minutes before the next collection cycle
        tokio::time::sleep(std::time::Duration::from_secs(15 * 60)).await;
    }
}

async fn fetch_repository_metrics(
    repo_url: &str,
    app_state: &web::Data<AppState>
) -> Result<RepositoryMetrics, String> {
    // Extract owner and repo from the URL
    // e.g., https://github.com/owner/repo
    let parts: Vec<&str> = repo_url.trim_end_matches('/').split('/').collect();
    if parts.len() < 5 {
        return Err("Invalid GitHub repo URL format".to_string());
    }

    let owner = parts[parts.len() - 2];
    let repo = parts[parts.len() - 1];

    let github_token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let mut headers = header::HeaderMap::new();

    if !github_token.is_empty() {
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("token {}", github_token))
                .map_err(|e| e.to_string())?
        );
    }

    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("service-metrics-collector")
    );

    // Now fetch the metrics
    let deployment_frequency = fetch_deployment_frequency(owner, repo, &app_state.http_client, &headers).await?;
    let lead_time = fetch_lead_time(owner, repo, &app_state.http_client, &headers).await?;
    let workflow_metrics = fetch_workflow_metrics(owner, repo, &app_state.http_client, &headers).await?;
    let deployment_tags = fetch_deployment_tags(owner, repo, &app_state.http_client, &headers).await?;

    Ok(RepositoryMetrics {
        deployment_frequency,
        lead_time,
        workflow_metrics,
        deployment_tags,
    })
}

async fn fetch_deployment_frequency(
    owner: &str,
    repo: &str,
    client: &reqwest::Client,
    headers: &header::HeaderMap,
) -> Result<DeploymentFrequency, String> {
    // Get deployments from the last 30 days
    let thirty_days_ago = (Utc::now() - Duration::days(30)).to_rfc3339();
    let url = format!(
        "{}/repos/{}/{}/deployments?since={}",
        GITHUB_API_BASE_URL, owner, repo, thirty_days_ago
    );

    let response = client
        .get(&url)
        .headers(headers.clone())
        .send()
        .await
        .map_err(|e| format!("Failed to fetch deployments: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned error: {}",
            response.status()
        ));
    }

    let deployments: Vec<GitHubDeployment> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse deployments: {}", e))?;

    Ok(DeploymentFrequency {
        last_30_days: deployments.len() as u32,
        deployments_per_week: (deployments.len() as f32 / 4.0) as f32,
        last_deployment: deployments
            .first()
            .map(|d| d.created_at)
            .unwrap_or_else(Utc::now),
    })
}

async fn fetch_lead_time(
    _owner: &str,
    _repo: &str,
    _client: &reqwest::Client,
    _headers: &header::HeaderMap,
) -> Result<LeadTimeForChanges, String> {
    // This is a simplified implementation - a real one would analyze PRs and their deployment times
    // For demonstration, we'll return placeholder data
    Ok(LeadTimeForChanges {
        average_lead_time_hours: 24.5,
        median_lead_time_hours: 18.2,
        ninety_percentile_hours: 72.0,
    })
}

async fn fetch_workflow_metrics(
    owner: &str,
    repo: &str,
    client: &reqwest::Client,
    headers: &header::HeaderMap,
) -> Result<WorkflowMetrics, String> {
    let url = format!(
        "{}/repos/{}/{}/actions/runs?per_page=20",
        GITHUB_API_BASE_URL, owner, repo
    );

    let response = client
        .get(&url)
        .headers(headers.clone())
        .send()
        .await
        .map_err(|e| format!("Failed to fetch workflow runs: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned error: {}",
            response.status()
        ));
    }

    let workflow_data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse workflow runs: {}", e))?;

    let runs = workflow_data["workflow_runs"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|run| {
            let status = run["status"].as_str().unwrap_or("unknown");
            let conclusion = run["conclusion"].as_str().unwrap_or("unknown");
            let created_at = run["created_at"].as_str()?;
            let updated_at = run["updated_at"].as_str()?;
            let name = run["name"].as_str().unwrap_or("unknown");

            Some(WorkflowRun {
                name: name.to_string(),
                status: status.to_string(),
                conclusion: conclusion.to_string(),
                created_at: DateTime::parse_from_rfc3339(created_at).ok()?.with_timezone(&Utc),
                duration_minutes: {
                    if let (Ok(start), Ok(end)) = (
                        DateTime::parse_from_rfc3339(created_at),
                        DateTime::parse_from_rfc3339(updated_at),
                    ) {
                        (end - start).num_minutes() as f32
                    } else {
                        0.0
                    }
                },
            })
        })
        .collect();

    Ok(WorkflowMetrics {
        recent_runs: runs,
        average_duration_minutes: calculate_average_duration(&runs),
    })
}

fn calculate_average_duration(runs: &[WorkflowRun]) -> f32 {
    if runs.is_empty() {
        return 0.0;
    }

    let sum: f32 = runs.iter().map(|r| r.duration_minutes).sum();
    sum / runs.len() as f32
}

async fn fetch_deployment_tags(
    _owner: &str,
    _repo: &str,
    _client: &reqwest::Client,
    _headers: &header::HeaderMap,
) -> Result<Vec<DeploymentTag>, String> {
    // In a real implementation, you'd parse deployment payloads to get image tags
    // For demonstration, we'll return placeholder data
    let tags = vec![
        DeploymentTag {
            environment: "production".to_string(),
            tag: "v1.2.3".to_string(),
            deployed_at: Utc::now(),
        },
        DeploymentTag {
            environment: "staging".to_string(),
            tag: "v1.2.4-rc1".to_string(),
            deployed_at: Utc::now() - Duration::days(1),
        },
    ];

    Ok(tags)
}
