use async_trait::async_trait;
use neusym_core::ports::CredentialResolver;
use neusym_core::{Credential, NeusymError, Provider, Result};

pub struct EnvCredentialResolver;

#[async_trait]
impl CredentialResolver for EnvCredentialResolver {
    async fn resolve(&self, provider: Provider) -> Result<Credential> {
        match provider {
            Provider::Linear => {
                let api_key = std::env::var("LINEAR_API_KEY").map_err(|_| {
                    NeusymError::MissingCredential {
                        field: "LINEAR_API_KEY".to_string(),
                    }
                })?;
                Ok(Credential::Linear { api_key })
            }
            Provider::Jira => {
                let base_url =
                    std::env::var("JIRA_BASE_URL").map_err(|_| NeusymError::MissingCredential {
                        field: "JIRA_BASE_URL".to_string(),
                    })?;
                let email =
                    std::env::var("JIRA_EMAIL").map_err(|_| NeusymError::MissingCredential {
                        field: "JIRA_EMAIL".to_string(),
                    })?;
                let api_token = std::env::var("JIRA_API_TOKEN").map_err(|_| {
                    NeusymError::MissingCredential {
                        field: "JIRA_API_TOKEN".to_string(),
                    }
                })?;
                Ok(Credential::Jira {
                    base_url,
                    email,
                    api_token,
                })
            }
        }
    }
}
