
#[derive(Clone)]
pub struct AppConfig {
    pub base_domain: String,
    pub internal_ns: Vec<String>, // "ns1.example.net.", ...
}
