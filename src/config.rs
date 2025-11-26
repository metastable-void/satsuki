#[derive(Clone)]
pub struct AppConfig {
    pub base_domain: String,
    pub internal_ns: Vec<String>, // "ns1.example.net.", ...
    pub internal_main_ns: String, // "ns1.example.net.", used in SOA
    pub internal_contact: String, // "hostmaster.example.net.", used in SOA
}

impl AppConfig {
    /// Canonical base domain without trailing dot.
    pub fn base_domain_root(&self) -> &str {
        self.base_domain.trim_end_matches('.')
    }

    /// Fully-qualified parent zone name (e.g. example.com.).
    pub fn parent_zone_name(&self) -> String {
        format!("{}.", self.base_domain_root())
    }

    /// Fully-qualified user zone name for the provided label.
    pub fn user_zone_name(&self, subdomain: &str) -> String {
        format!("{}.{}.", subdomain, self.base_domain_root())
    }
}
