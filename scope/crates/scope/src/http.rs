use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use url::Url;

use crate::config::HttpConfig;

pub struct HttpClient {
    client: reqwest::Client,
    max_body_bytes: u64,
}

#[derive(Debug)]
pub struct HttpResponse {
    pub url: Url,
    pub content_type: Option<String>,
    pub body: String,
}

impl HttpClient {
    pub fn from_config(http: &HttpConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(http.timeout_secs))
            .user_agent(&http.user_agent)
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            client,
            max_body_bytes: http.max_body_bytes,
        })
    }

    pub async fn fetch(&self, url: &Url) -> Result<HttpResponse> {
        let mut response = self
            .client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("requesting {url}"))?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow!("HTTP {} for {}", status.as_u16(), url));
        }

        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        let mut buffer = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .with_context(|| format!("reading body from {final_url}"))?
        {
            if buffer.len() as u64 + chunk.len() as u64 > self.max_body_bytes {
                return Err(anyhow!(
                    "body for {} exceeds max_body_bytes ({})",
                    final_url,
                    self.max_body_bytes
                ));
            }
            buffer.extend_from_slice(&chunk);
        }

        let body = String::from_utf8(buffer)
            .with_context(|| format!("decoding body from {final_url} as UTF-8"))?;

        Ok(HttpResponse {
            url: final_url,
            content_type,
            body,
        })
    }
}

#[cfg(test)]
pub(crate) fn test_client() -> std::sync::Arc<HttpClient> {
    std::sync::Arc::new(
        HttpClient::from_config(&HttpConfig {
            timeout_secs: 5,
            max_body_bytes: 1_000_000,
            user_agent: "scope-test/1.0".into(),
        })
        .unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn config_with(max_body_bytes: u64, user_agent: &str) -> HttpConfig {
        HttpConfig {
            timeout_secs: 5,
            max_body_bytes,
            user_agent: user_agent.to_string(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetches_successful_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/page"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw("<html>hi</html>".as_bytes().to_vec(), "text/html; charset=utf-8"),
            )
            .mount(&server)
            .await;

        let client = HttpClient::from_config(&config_with(1_000_000, "scope-test/1.0")).unwrap();
        let url = Url::parse(&format!("{}/page", server.uri())).unwrap();
        let response = client.fetch(&url).await.unwrap();

        assert_eq!(response.body, "<html>hi</html>");
        assert_eq!(
            response.content_type.as_deref(),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(response.url, url);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn errors_on_non_success_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = HttpClient::from_config(&config_with(1_000_000, "scope-test/1.0")).unwrap();
        let url = Url::parse(&format!("{}/missing", server.uri())).unwrap();
        let err = client.fetch(&url).await.unwrap_err();
        assert!(err.to_string().contains("404"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn errors_when_body_exceeds_max_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(100)))
            .mount(&server)
            .await;

        let client = HttpClient::from_config(&config_with(10, "scope-test/1.0")).unwrap();
        let url = Url::parse(&format!("{}/big", server.uri())).unwrap();
        let err = client.fetch(&url).await.unwrap_err();
        assert!(err.to_string().contains("max_body_bytes"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sends_configured_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ua"))
            .and(header("user-agent", "scope-test/9.9"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = HttpClient::from_config(&config_with(1_000, "scope-test/9.9")).unwrap();
        let url = Url::parse(&format!("{}/ua", server.uri())).unwrap();
        let response = client.fetch(&url).await.unwrap();
        assert_eq!(response.body, "ok");
    }
}
