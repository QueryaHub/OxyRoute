//! Compiled Security Headers configuration (issues #54, #218).

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Compiled native representation of `SecurityHeadersConfig`.
#[derive(Clone, Debug)]
pub struct CompiledSecurityHeadersConfig {
    pub hsts: Option<String>,
    pub x_content_type_options: Option<String>,
    pub x_frame_options: Option<String>,
    pub referrer_policy: Option<String>,
    pub content_security_policy: Option<String>,
    pub permissions_policy: Option<String>,
    pub extra: Vec<(String, String)>,
}

impl CompiledSecurityHeadersConfig {
    /// Attempt to compile a Python `SecurityHeadersConfig` object into a native Rust struct.
    pub fn from_py(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let hsts: Option<String> = obj.getattr("hsts")?.extract()?;
        let x_content_type_options: Option<String> =
            obj.getattr("x_content_type_options")?.extract()?;
        let x_frame_options: Option<String> = obj.getattr("x_frame_options")?.extract()?;
        let referrer_policy: Option<String> = obj.getattr("referrer_policy")?.extract()?;
        let content_security_policy: Option<String> =
            obj.getattr("content_security_policy")?.extract()?;
        let permissions_policy: Option<String> = obj.getattr("permissions_policy")?.extract()?;

        let mut extra = Vec::new();
        if let Ok(extra_attr) = obj.getattr("extra") {
            if let Ok(extra_dict) = extra_attr.downcast::<PyDict>() {
                for (k, v) in extra_dict.iter() {
                    let key: String = k.extract()?;
                    let val: String = v.extract()?;
                    extra.push((key, val));
                }
            }
        }

        Ok(Self {
            hsts,
            x_content_type_options,
            x_frame_options,
            referrer_policy,
            content_security_policy,
            permissions_policy,
            extra,
        })
    }

    /// Generates security headers to merge into outgoing responses.
    pub fn response_header_pairs(&self, scheme: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if scheme == "https" {
            if let Some(ref hsts) = self.hsts {
                out.push(("Strict-Transport-Security".to_string(), hsts.clone()));
            }
        }
        if let Some(ref v) = self.x_content_type_options {
            out.push(("X-Content-Type-Options".to_string(), v.clone()));
        }
        if let Some(ref v) = self.x_frame_options {
            out.push(("X-Frame-Options".to_string(), v.clone()));
        }
        if let Some(ref v) = self.referrer_policy {
            out.push(("Referrer-Policy".to_string(), v.clone()));
        }
        if let Some(ref v) = self.content_security_policy {
            out.push(("Content-Security-Policy".to_string(), v.clone()));
        }
        if let Some(ref v) = self.permissions_policy {
            out.push(("Permissions-Policy".to_string(), v.clone()));
        }
        for (k, v) in &self.extra {
            out.push((k.clone(), v.clone()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsts_only_on_https() {
        let cfg = CompiledSecurityHeadersConfig {
            hsts: Some("max-age=31536000".to_string()),
            x_content_type_options: Some("nosniff".to_string()),
            x_frame_options: Some("DENY".to_string()),
            referrer_policy: None,
            content_security_policy: None,
            permissions_policy: None,
            extra: vec![],
        };
        let http_pairs = cfg.response_header_pairs("http");
        assert!(!http_pairs
            .iter()
            .any(|(k, _)| k == "Strict-Transport-Security"));
        assert!(http_pairs
            .iter()
            .any(|(k, v)| k == "X-Content-Type-Options" && v == "nosniff"));

        let https_pairs = cfg.response_header_pairs("https");
        assert!(https_pairs
            .iter()
            .any(|(k, v)| k == "Strict-Transport-Security" && v == "max-age=31536000"));
    }
}
