use serde::{Deserialize, Serialize};

/// Logsource represents the logsource field in sigma rule
/// It defines relevant event streams and is used for pre-filtering
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Logsource {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Product name (e.g., windows, linux)
    pub product: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Log category (e.g., process_creation, network_connection)
    pub category: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Service name (e.g., sysmon, security)
    pub service: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Custom definition for log source
    pub definition: Option<String>,
}

impl Logsource {
    /// Check if this logsource matches the provided filters
    pub fn matches(&self, product: Option<&str>, category: Option<&str>, service: Option<&str>) -> bool {
        let product_match = match (product, &self.product) {
            (Some(p), Some(self_p)) => p == self_p,
            (None, _) => true,
            (Some(_), None) => false,
        };
        
        let category_match = match (category, &self.category) {
            (Some(c), Some(self_c)) => c == self_c,
            (None, _) => true,
            (Some(_), None) => false,
        };
        
        let service_match = match (service, &self.service) {
            (Some(s), Some(self_s)) => s == self_s,
            (None, _) => true,
            (Some(_), None) => false,
        };
        
        product_match && category_match && service_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_logsource_matches() {
        let logsource = Logsource {
            product: Some("windows".to_string()),
            category: Some("process_creation".to_string()),
            service: None,
            definition: None,
        };
        
        // Should match when filters match
        assert!(logsource.matches(
            Some("windows"),
            Some("process_creation"),
            None
        ));
        
        // Should not match when product differs
        assert!(!logsource.matches(
            Some("linux"),
            Some("process_creation"),
            None
        ));
        
        // Should match when no filters provided
        assert!(logsource.matches(None, None, None));
    }
}