use crate::error::CliError;
use reqwest::Url;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DaemonClient {
    base_url: Url,
    inner: reqwest::Client,
}

impl DaemonClient {
    pub fn new(base_url: String) -> Result<Self, CliError> {
        let normalized = if base_url.ends_with('/') {
            base_url
        } else {
            format!("{base_url}/")
        };
        let base_url =
            Url::parse(&normalized).map_err(|_| CliError::InvalidDaemonUrl(normalized.clone()))?;
        Ok(Self {
            base_url,
            inner: reqwest::Client::new(),
        })
    }

    pub async fn get_value(&self, path: &str) -> Result<Value, CliError> {
        self.get(path).await
    }

    pub async fn get<T>(&self, path: &str) -> Result<T, CliError>
    where
        T: DeserializeOwned,
    {
        let response = self
            .inner
            .get(self.url(path)?)
            .send()
            .await
            .map_err(|source| CliError::DaemonUnavailable {
                url: self.base_url.to_string(),
                source,
            })?;

        self.decode_response(path, response).await
    }

    pub async fn post_json<T, B>(&self, path: &str, body: &B) -> Result<T, CliError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let response = self
            .inner
            .post(self.url(path)?)
            .json(body)
            .send()
            .await
            .map_err(|source| CliError::DaemonUnavailable {
                url: self.base_url.to_string(),
                source,
            })?;

        self.decode_response(path, response).await
    }

    pub async fn post_json_empty<B>(&self, path: &str, body: &B) -> Result<(), CliError>
    where
        B: Serialize + ?Sized,
    {
        let response = self
            .inner
            .post(self.url(path)?)
            .json(body)
            .send()
            .await
            .map_err(|source| CliError::DaemonUnavailable {
                url: self.base_url.to_string(),
                source,
            })?;

        self.ensure_success(path, response).await
    }

    pub async fn post_empty(&self, path: &str) -> Result<(), CliError> {
        let response = self
            .inner
            .post(self.url(path)?)
            .send()
            .await
            .map_err(|source| CliError::DaemonUnavailable {
                url: self.base_url.to_string(),
                source,
            })?;

        self.ensure_success(path, response).await
    }

    fn url(&self, path: &str) -> Result<Url, CliError> {
        let path = path.trim_start_matches('/');
        self.base_url
            .join(path)
            .map_err(|_| CliError::InvalidDaemonUrl(self.base_url.to_string()))
    }

    async fn decode_response<T>(
        &self,
        path: &str,
        response: reqwest::Response,
    ) -> Result<T, CliError>
    where
        T: DeserializeOwned,
    {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(CliError::DaemonStatus {
                status,
                path: path.to_string(),
                body,
            });
        }

        response
            .json::<T>()
            .await
            .map_err(|source| CliError::Decode {
                path: path.to_string(),
                source,
            })
    }

    async fn ensure_success(
        &self,
        path: &str,
        response: reqwest::Response,
    ) -> Result<(), CliError> {
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(CliError::DaemonStatus {
                status,
                path: path.to_string(),
                body,
            })
        }
    }
}
