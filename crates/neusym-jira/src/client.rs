use async_trait::async_trait;
use neusym_core::{NeusymError, NormalizedIssue, Provider, Result, ports::IssueProvider};
use reqwest::Client;

pub struct JiraClient {
    client: Client,
    base_url: String,
    email: String,
    api_token: String,
}

impl JiraClient {
    pub fn new(base_url: String, email: String, api_token: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            email,
            api_token,
        }
    }

    fn parse_issue(&self, i: &serde_json::Value) -> NormalizedIssue {
        let fields = &i["fields"];
        NormalizedIssue {
            provider: Provider::Jira,
            id: i["id"].as_str().unwrap_or_default().to_string(),
            identifier: i["key"].as_str().unwrap_or_default().to_string(),
            title: fields["summary"].as_str().unwrap_or_default().to_string(),
            description: fields["description"].as_str().map(String::from),
            status: fields["status"]["name"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string(),
            priority: fields["priority"]["name"].as_str().map(String::from),
            labels: fields["labels"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|l| l.as_str().map(String::from))
                .collect(),
            assignee: fields["assignee"]["displayName"].as_str().map(String::from),
            parent_id: fields["parent"]["key"].as_str().map(String::from),
            url: Some(format!(
                "{}/browse/{}",
                self.base_url,
                i["key"].as_str().unwrap_or_default()
            )),
        }
    }
}

#[async_trait]
impl IssueProvider for JiraClient {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedIssue>> {
        let jql = format!("summary ~ \"{}\" ORDER BY updated DESC", query);
        let url = format!("{}/rest/api/3/search", self.base_url);
        let resp = self
            .client
            .get(&url)
            .basic_auth(&self.email, Some(&self.api_token))
            .query(&[("jql", &jql), ("maxResults", &"50".to_string())])
            .send()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        let issues = json["issues"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|i| self.parse_issue(i))
            .collect();
        Ok(issues)
    }

    async fn get(&self, identifier: &str) -> Result<NormalizedIssue> {
        let url = format!("{}/rest/api/3/issue/{}", self.base_url, identifier);
        let resp = self
            .client
            .get(&url)
            .basic_auth(&self.email, Some(&self.api_token))
            .send()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        let i: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        Ok(self.parse_issue(&i))
    }

    async fn create(&self, issue: &NormalizedIssue) -> Result<NormalizedIssue> {
        let url = format!("{}/rest/api/3/issue", self.base_url);
        let mut fields = serde_json::json!({
            "summary": issue.title,
            "issuetype": { "name": "Task" },
        });
        if let Some(ref desc) = issue.description {
            fields["description"] = serde_json::json!({
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": desc,
                    }]
                }]
            });
        }
        if !issue.labels.is_empty() {
            fields["labels"] = serde_json::json!(issue.labels);
        }
        let body = serde_json::json!({ "fields": fields });
        let resp = self
            .client
            .post(&url)
            .basic_auth(&self.email, Some(&self.api_token))
            .json(&body)
            .send()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        let created: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        let key = created["key"]
            .as_str()
            .ok_or_else(|| NeusymError::Provider("Jira create: no key in response".to_string()))?;
        self.get(key).await
    }

    async fn update(&self, identifier: &str, issue: &NormalizedIssue) -> Result<NormalizedIssue> {
        let url = format!("{}/rest/api/3/issue/{}", self.base_url, identifier);
        let mut fields = serde_json::json!({ "summary": issue.title });
        if let Some(ref desc) = issue.description {
            fields["description"] = serde_json::json!({
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": desc,
                    }]
                }]
            });
        }
        if !issue.labels.is_empty() {
            fields["labels"] = serde_json::json!(issue.labels);
        }
        let body = serde_json::json!({ "fields": fields });
        self.client
            .put(&url)
            .basic_auth(&self.email, Some(&self.api_token))
            .json(&body)
            .send()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        self.get(identifier).await
    }
}
