use crate::powerdns::types::*;
use reqwest::Client;
use serde::Serialize;

#[derive(Clone)]
pub struct PowerDnsClient {
    http: Client,
    base_url: String, // e.g. "http://127.0.0.1:8081/api/v1"
    api_key: String,
    server_id: String, // usually "localhost"
}

impl PowerDnsClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        server_id: impl Into<String>,
    ) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            server_id: server_id.into(),
        }
    }

    fn auth_header(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("X-API-Key", &self.api_key)
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/servers/{}/{}",
            self.base_url,
            self.server_id,
            path.trim_start_matches('/')
        )
    }

    pub async fn get_zone(&self, name: &str) -> anyhow::Result<PdnsZone> {
        let url = self.url(&format!("zones/{}", name));
        let res = self.auth_header(self.http.get(url)).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("PowerDNS get_zone failed with {}", res.status());
        }
        Ok(res.json::<PdnsZone>().await?)
    }

    pub async fn create_zone(&self, z: &PdnsZoneCreate) -> anyhow::Result<()> {
        let url = self.url("zones");
        let res = self.auth_header(self.http.post(url)).json(z).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("PowerDNS create_zone failed with {}", res.status());
        }
        Ok(())
    }

    pub async fn patch_rrsets(&self, zone_name: &str, rrsets: &[PdnsRrset]) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct PatchBody<'a> {
            rrsets: &'a [PdnsRrset],
        }

        let url = self.url(&format!("zones/{}", zone_name));
        let body = PatchBody { rrsets };
        let res = self
            .auth_header(self.http.patch(url))
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            anyhow::bail!("PowerDNS patch_rrsets failed with {}", res.status());
        }
        Ok(())
    }

    pub async fn delete_zone(&self, name: &str) -> anyhow::Result<()> {
        let url = self.url(&format!("zones/{}", name));
        let res = self.auth_header(self.http.delete(url)).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("PowerDNS delete_zone failed with {}", res.status());
        }
        Ok(())
    }
}
