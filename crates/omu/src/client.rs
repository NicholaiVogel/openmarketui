use crate::error::CliError;
use reqwest::{header::HeaderValue, RequestBuilder, Url};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

const TRACE_ID_HEADER: &str = "x-omu-trace-id";

#[derive(Debug, Clone)]
pub struct DaemonClient {
    base_url: Url,
    inner: reqwest::Client,
    trace_id: Option<String>,
}

impl DaemonClient {
    pub fn new(base_url: String, trace_id: Option<String>) -> Result<Self, CliError> {
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
            trace_id,
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
            .with_trace_header(self.inner.get(self.url(path)?))?
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
            .with_trace_header(self.inner.post(self.url(path)?))?
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
            .with_trace_header(self.inner.post(self.url(path)?))?
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
            .with_trace_header(self.inner.post(self.url(path)?))?
            .send()
            .await
            .map_err(|source| CliError::DaemonUnavailable {
                url: self.base_url.to_string(),
                source,
            })?;

        self.ensure_success(path, response).await
    }

    fn with_trace_header(&self, builder: RequestBuilder) -> Result<RequestBuilder, CliError> {
        let Some(trace_id) = self.trace_id.as_deref() else {
            return Ok(builder);
        };
        let value = HeaderValue::from_str(trace_id).map_err(|_| CliError::InvalidArgument {
            message: "trace ID contains characters that are not valid in an HTTP header"
                .to_string(),
        })?;
        Ok(builder.header(TRACE_ID_HEADER, value))
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
