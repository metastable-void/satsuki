//! Static application configuration and helpers around DNS naming.
use std::borrow::Cow;

/// Default label blacklist applied when no custom list is supplied.
pub const DEFAULT_DISALLOWED_SUBDOMAINS: &[&str] = &[
    // Common service labels that should remain reserved for infrastructure hosts
    "www",
    "mail",
    "email",
    "ftp",
    "smtp",
    "imap",
    "pop",
    "pop3",
    "mx",
    "ns",
    "autodiscover",
    "autoconfig",
    // RFC 2606 / 6761 special-use labels
    "example",
    "invalid",
    "localhost",
    "test",
];

/// Strongly-typed representation of server configuration.
#[derive(Clone)]
pub struct AppConfig {
    pub base_domain: String,
    pub internal_ns: Vec<String>, // "ns1.example.net.", ...
    pub internal_main_ns: String, // "ns1.example.net.", used in SOA
    pub internal_contact: String, // "hostmaster.example.net.", used in SOA
    pub disallowed_subdomains: Vec<String>,
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

    /// Check whether the user-provided label is on the reserved list.
    pub fn is_disallowed_subdomain(&self, label: &str) -> bool {
        let needle = label.to_ascii_lowercase();
        self.effective_disallowed_subdomains()
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(&needle))
    }

    /// Return either the custom list or the baked-in default.
    pub fn effective_disallowed_subdomains(&self) -> Cow<'_, [String]> {
        if self.disallowed_subdomains.is_empty() {
            Cow::Owned(
                DEFAULT_DISALLOWED_SUBDOMAINS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            )
        } else {
            Cow::Borrowed(&self.disallowed_subdomains)
        }
    }
}
