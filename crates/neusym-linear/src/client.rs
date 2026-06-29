use async_trait::async_trait;
use neusym_core::{NeusymError, NormalizedIssue, Provider, Result, ports::IssueProvider};
use reqwest::Client;

pub struct LinearClient {
    client: Client,
    api_key: String,
}

impl LinearClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    async fn graphql(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body = serde_json::json!({ "query": query, "variables": variables });
        let resp = self
            .client
            .post("https://api.linear.app/graphql")
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| NeusymError::Http(e.to_string()))?;

        Ok(json)
    }

    fn parse_issue(n: &serde_json::Value) -> NormalizedIssue {
        NormalizedIssue {
            provider: Provider::Linear,
            id: n["id"].as_str().unwrap_or_default().to_string(),
            identifier: n["identifier"].as_str().unwrap_or_default().to_string(),
            title: n["title"].as_str().unwrap_or_default().to_string(),
            description: n["description"].as_str().map(String::from),
            status: n["state"]["name"].as_str().unwrap_or("Unknown").to_string(),
            priority: n["priorityLabel"].as_str().map(String::from),
            labels: n["labels"]["nodes"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|l| l["name"].as_str().map(String::from))
                .collect(),
            assignee: n["assignee"]["name"].as_str().map(String::from),
            parent_id: n["parent"]["identifier"].as_str().map(String::from),
            url: n["url"].as_str().map(String::from),
        }
    }

    fn priority_to_int(label: &str) -> u8 {
        match label.to_lowercase().as_str() {
            "urgent" => 1,
            "high" => 2,
            "medium" => 3,
            "low" => 4,
            _ => 0,
        }
    }
}

const ISSUE_FIELDS: &str = "id identifier title description \
    state { name } priority priorityLabel \
    labels { nodes { name } } \
    assignee { name } parent { identifier } \
    url";

#[async_trait]
impl IssueProvider for LinearClient {
    async fn search(&self, query: &str) -> Result<Vec<NormalizedIssue>> {
        let gql = format!(
            "query($filter: IssueFilter) {{ issues(filter: $filter, first: 50) {{ nodes {{ {} }} }} }}",
            ISSUE_FIELDS
        );
        let variables = serde_json::json!({
            "filter": { "title": { "contains": query } }
        });
        let data = self.graphql(&gql, variables).await?;
        let nodes = &data["data"]["issues"]["nodes"];
        let issues = nodes
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(Self::parse_issue)
            .collect();
        Ok(issues)
    }

    async fn get(&self, identifier: &str) -> Result<NormalizedIssue> {
        let gql = format!(
            "query($filter: IssueFilter) {{ issues(filter: $filter, first: 1) {{ nodes {{ {} }} }} }}",
            ISSUE_FIELDS
        );
        let variables = serde_json::json!({ "filter": { "identifier": { "eq": identifier } } });
        let data = self.graphql(&gql, variables).await?;
        let node = &data["data"]["issues"]["nodes"][0];
        if node.is_null() {
            return Err(NeusymError::Provider(format!(
                "Linear issue not found: {identifier}"
            )));
        }
        Ok(Self::parse_issue(node))
    }

    async fn create(&self, issue: &NormalizedIssue) -> Result<NormalizedIssue> {
        let gql = format!(
            "mutation($input: IssueCreateInput!) {{ issueCreate(input: $input) {{ success issue {{ {} }} }} }}",
            ISSUE_FIELDS
        );
        let mut input = serde_json::json!({ "title": issue.title });
        if let Some(ref desc) = issue.description {
            input["description"] = serde_json::json!(desc);
        }
        if let Some(ref priority) = issue.priority {
            input["priority"] = serde_json::json!(Self::priority_to_int(priority));
        }
        let variables = serde_json::json!({ "input": input });
        let data = self.graphql(&gql, variables).await?;
        if data["data"]["issueCreate"]["success"]
            .as_bool()
            .unwrap_or(false)
        {
            Ok(Self::parse_issue(&data["data"]["issueCreate"]["issue"]))
        } else {
            Err(NeusymError::Provider(
                "Linear issueCreate failed".to_string(),
            ))
        }
    }

    async fn ping(&self) -> Result<()> {
        let data = self
            .graphql("query { viewer { id } }", serde_json::json!({}))
            .await?;
        if data["data"]["viewer"]["id"].is_null() {
            return Err(NeusymError::Provider("Linear viewer query failed".into()));
        }
        Ok(())
    }

    async fn update(&self, identifier: &str, issue: &NormalizedIssue) -> Result<NormalizedIssue> {
        let gql = format!(
            "mutation($id: String!, $input: IssueUpdateInput!) {{ issueUpdate(id: $id, input: $input) {{ success issue {{ {} }} }} }}",
            ISSUE_FIELDS
        );
        let mut input = serde_json::json!({ "title": issue.title });
        if let Some(ref desc) = issue.description {
            input["description"] = serde_json::json!(desc);
        }
        if let Some(ref priority) = issue.priority {
            input["priority"] = serde_json::json!(Self::priority_to_int(priority));
        }
        let variables = serde_json::json!({
            "id": identifier,
            "input": input,
        });
        let data = self.graphql(&gql, variables).await?;
        if data["data"]["issueUpdate"]["success"]
            .as_bool()
            .unwrap_or(false)
        {
            Ok(Self::parse_issue(&data["data"]["issueUpdate"]["issue"]))
        } else {
            Err(NeusymError::Provider(
                "Linear issueUpdate failed".to_string(),
            ))
        }
    }
}
