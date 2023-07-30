use std::time::Duration;

use reqwest::{ClientBuilder, Proxy, Method, header::HeaderMap, multipart};
use anyhow::Result;
pub(crate) fn client_builder_with_proxies(
    proxys: &[Proxy],
    mut client_builder: ClientBuilder,
) -> ClientBuilder {
    for proxy in proxys.iter() {
        client_builder = client_builder.proxy(proxy.clone());
    }
    client_builder
}

pub(crate) async fn request(
    method: Method,
    url: &str,
    proxys: &[Proxy],
    headers: HeaderMap,
    body: Option<String>,
    form: Option<multipart::Form>,
) -> Result<bytes::Bytes> {
    let client_builder = client_builder_with_proxies(
        proxys,
        reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10)),
    );

    let client = client_builder.build()?;
    let mut req = client.request(method, url);
    if let Some(body) = body {
        req = req.body(body);
    }
    if let Some(form) = form {
        req = req.multipart(form);
    }
    let res = req.send().await?.bytes().await.map_err(Into::into);

    log::debug!(
        "url:{:?}, response:{:?}",
        url,
        String::from_utf8_lossy(&res.as_ref().unwrap_or(&bytes::Bytes::default()))
    );

    res
}