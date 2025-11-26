use regex::Regex;

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("subdomain is empty")]
    Empty,
    #[error("subdomain too long (max 63 characters)")]
    TooLong,
    #[error("subdomain contains invalid characters (only a-z, 0-9, and '-' allowed)")]
    InvalidCharacters,
    #[error("subdomain must not start or end with '-'")]
    LeadingOrTrailingHyphen,
    #[error("subdomain must not contain consecutive '--'")]
    DoubleHyphen,
}

lazy_static::lazy_static! {
    /// Only lowercase letters, digits and '-'
    static ref SUBDOMAIN_RE: Regex = Regex::new(r"^[a-z0-9-]+$").unwrap();
}

pub fn validate_subdomain_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::Empty);
    }
    if name.len() > 63 {
        return Err(ValidationError::TooLong);
    }
    if !SUBDOMAIN_RE.is_match(name) {
        return Err(ValidationError::InvalidCharacters);
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err(ValidationError::LeadingOrTrailingHyphen);
    }
    if name.contains("--") {
        return Err(ValidationError::DoubleHyphen);
    }

    Ok(())
}

pub fn validate_fqdn_ascii(domain: &str) -> Result<(), ValidationError> {
    // require trailing dot for clarity, or add it yourself
    let d = domain.trim_end_matches('.');
    if d.is_empty() {
        return Err(ValidationError::Empty);
    }
    for label in d.split('.') {
        validate_subdomain_name(label)?;
    }
    Ok(())
}
