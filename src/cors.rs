//! Compiled CORS configuration and native preflight handling (issues #49, #108, #218).

use std::collections::HashSet;

use pyo3::prelude::*;

pub type PreflightResult = Result<Vec<(String, String)>, (u16, &'static str)>;

/// Compiled native representation of `CORSConfig`.
#[derive(Clone, Debug)]
pub struct CompiledCorsConfig {
    pub allow_origins: Vec<String>,
    pub allow_origins_wildcard: bool,
    pub allow_methods_upper: HashSet<String>,
    pub allow_methods_header: String,
    pub allow_headers: Vec<String>,
    pub allow_headers_wildcard: bool,
    pub allow_headers_lower: HashSet<String>,
    pub expose_headers_header: Option<String>,
    pub allow_credentials: bool,
    pub max_age_header: Option<String>,
}

impl CompiledCorsConfig {
    /// Attempt to compile a Python `CORSConfig` object into a native Rust struct.
    pub fn from_py(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let allow_origins: Vec<String> = obj.getattr("allow_origins")?.extract()?;
        let allow_methods: Vec<String> = obj.getattr("allow_methods")?.extract()?;
        let allow_headers: Vec<String> = obj.getattr("allow_headers")?.extract()?;
        let expose_headers: Vec<String> = obj.getattr("expose_headers")?.extract()?;
        let allow_credentials: bool = obj.getattr("allow_credentials")?.extract()?;
        let max_age: Option<u64> = obj.getattr("max_age")?.extract()?;

        let allow_origins_wildcard = allow_origins.iter().any(|o| o == "*");
        let allow_methods_upper: HashSet<String> = allow_methods
            .iter()
            .map(|m| m.to_ascii_uppercase())
            .collect();
        let allow_methods_header = allow_methods.join(", ");
        let allow_headers_wildcard = allow_headers.iter().any(|h| h == "*");
        let allow_headers_lower: HashSet<String> = allow_headers
            .iter()
            .filter(|h| *h != "*")
            .map(|h| h.to_ascii_lowercase())
            .collect();
        let expose_headers_header = if expose_headers.is_empty() {
            None
        } else {
            Some(expose_headers.join(", "))
        };
        let max_age_header = max_age.map(|a| a.to_string());

        Ok(Self {
            allow_origins,
            allow_origins_wildcard,
            allow_methods_upper,
            allow_methods_header,
            allow_headers,
            allow_headers_wildcard,
            allow_headers_lower,
            expose_headers_header,
            allow_credentials,
            max_age_header,
        })
    }

    /// Selects the allowed origin for the response, matching `CORSConfig._select_allow_origin`.
    #[inline]
    pub fn select_allow_origin<'a>(&'a self, origin: &'a str) -> Option<&'a str> {
        if origin.is_empty() {
            return None;
        }
        if self.allow_credentials || !self.allow_origins_wildcard {
            if self.allow_origins.iter().any(|o| o == origin) {
                return Some(origin);
            }
            return None;
        }
        if self.allow_origins.len() == 1 && self.allow_origins_wildcard {
            return Some("*");
        }
        if self.allow_origins.iter().any(|o| o == origin) {
            return Some(origin);
        }
        None
    }

    /// Computes the `Access-Control-Allow-Headers` response value for preflight.
    pub fn preflight_allow_headers(&self, raw_ach: &str) -> Option<String> {
        if self.allow_headers_wildcard && self.allow_headers.len() == 1 {
            return Some("*".to_string());
        }
        let parts: Vec<&str> = raw_ach
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.is_empty() {
            return Some(self.allow_headers.join(", "));
        }
        for p in &parts {
            if !self.allow_headers_lower.contains(&p.to_ascii_lowercase()) {
                return None;
            }
        }
        Some(parts.join(", "))
    }

    /// Handles a CORS preflight request natively.
    /// Returns `Ok(preflight_headers)` on success (for a 204 response),
    /// or `Err((status, error_message))` (for a 400 response).
    pub fn handle_preflight(&self, raw_acrm: &str, origin: &str, raw_ach: &str) -> PreflightResult {
        let request_method = raw_acrm.trim().to_ascii_uppercase();
        if !self.allow_methods_upper.contains(&request_method) {
            return Err((400, "Disallowed CORS method"));
        }
        let selected = self
            .select_allow_origin(origin)
            .ok_or((400, "CORS origin not allowed"))?;
        let acah = self
            .preflight_allow_headers(raw_ach)
            .ok_or((400, "Disallowed CORS request headers"))?;

        let mut headers = vec![
            (
                "Access-Control-Allow-Origin".to_string(),
                selected.to_string(),
            ),
            (
                "Access-Control-Allow-Methods".to_string(),
                self.allow_methods_header.clone(),
            ),
            ("Access-Control-Allow-Headers".to_string(), acah),
        ];
        if self.allow_credentials {
            headers.push((
                "Access-Control-Allow-Credentials".to_string(),
                "true".to_string(),
            ));
        }
        if let Some(ref max_age) = self.max_age_header {
            headers.push(("Access-Control-Max-Age".to_string(), max_age.clone()));
        }
        if selected != "*" {
            headers.push(("Vary".to_string(), "Origin".to_string()));
        }
        Ok(headers)
    }

    /// Generates CORS headers to merge into a normal (non-preflight) response.
    pub fn response_header_pairs(&self, origin: &str) -> Vec<(String, String)> {
        let Some(selected) = self.select_allow_origin(origin) else {
            return Vec::new();
        };
        let mut out = vec![(
            "Access-Control-Allow-Origin".to_string(),
            selected.to_string(),
        )];
        if self.allow_credentials {
            out.push((
                "Access-Control-Allow-Credentials".to_string(),
                "true".to_string(),
            ));
        }
        if let Some(ref exp) = self.expose_headers_header {
            out.push(("Access-Control-Expose-Headers".to_string(), exp.clone()));
        }
        if selected != "*" {
            out.push(("Vary".to_string(), "Origin".to_string()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_allow_origin() {
        let cfg = CompiledCorsConfig {
            allow_origins: vec!["https://a.example".to_string()],
            allow_origins_wildcard: false,
            allow_methods_upper: HashSet::new(),
            allow_methods_header: String::new(),
            allow_headers: vec!["*".to_string()],
            allow_headers_wildcard: true,
            allow_headers_lower: HashSet::new(),
            expose_headers_header: None,
            allow_credentials: false,
            max_age_header: None,
        };
        assert_eq!(cfg.select_allow_origin(""), None);
        assert_eq!(cfg.select_allow_origin("https://b.example"), None);
        assert_eq!(
            cfg.select_allow_origin("https://a.example"),
            Some("https://a.example")
        );
    }

    #[test]
    fn test_preflight_success() {
        let mut methods = HashSet::new();
        methods.insert("POST".to_string());
        let cfg = CompiledCorsConfig {
            allow_origins: vec!["https://app.example".to_string()],
            allow_origins_wildcard: false,
            allow_methods_upper: methods,
            allow_methods_header: "POST".to_string(),
            allow_headers: vec!["X-Custom".to_string()],
            allow_headers_wildcard: false,
            allow_headers_lower: ["x-custom".to_string()].into_iter().collect(),
            expose_headers_header: None,
            allow_credentials: true,
            max_age_header: Some("600".to_string()),
        };
        let res = cfg.handle_preflight("POST", "https://app.example", "x-custom");
        assert!(res.is_ok());
        let hdrs = res.unwrap();
        assert!(hdrs
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Origin" && v == "https://app.example"));
        assert!(hdrs
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Credentials" && v == "true"));
        assert!(hdrs
            .iter()
            .any(|(k, v)| k == "Access-Control-Max-Age" && v == "600"));
        assert!(hdrs.iter().any(|(k, v)| k == "Vary" && v == "Origin"));
    }

    #[test]
    fn test_preflight_disallowed_method() {
        let cfg = CompiledCorsConfig {
            allow_origins: vec!["https://app.example".to_string()],
            allow_origins_wildcard: false,
            allow_methods_upper: HashSet::new(),
            allow_methods_header: String::new(),
            allow_headers: vec!["*".to_string()],
            allow_headers_wildcard: true,
            allow_headers_lower: HashSet::new(),
            expose_headers_header: None,
            allow_credentials: false,
            max_age_header: None,
        };
        let res = cfg.handle_preflight("PUT", "https://app.example", "");
        assert_eq!(res, Err((400, "Disallowed CORS method")));
    }
}
