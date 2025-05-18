use serde::{Deserialize, Serialize};

/// Result is an object returned on positive sigma match
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Result {
    #[serde(default)]
    pub tags: Vec<String>,
    
    pub id: String,
    pub title: String,
    pub description: String,
}

impl Result {
    /// Create a new Result
    pub fn new(id: String, title: String, description: String) -> Self {
        Self {
            tags: Vec::new(),
            id,
            title,
            description,
        }
    }
    
    /// Create a Result with tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Results should be returned when single event matches multiple rules
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Results(pub Vec<Result>);

impl Results {
    /// Create a new empty Results collection
    pub fn new() -> Self {
        Self(Vec::new())
    }
    
    /// Add a result to the collection
    pub fn push(&mut self, result: Result) {
        self.0.push(result);
    }
    
    /// Check if there are any results
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    
    /// Get the number of results
    pub fn len(&self) -> usize {
        self.0.len()
    }
    
    /// Get an iterator over the results
    pub fn iter(&self) -> std::slice::Iter<Result> {
        self.0.iter()
    }
}

impl IntoIterator for Results {
    type Item = Result;
    type IntoIter = std::vec::IntoIter<Result>;
    
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Results {
    type Item = &'a Result;
    type IntoIter = std::slice::Iter<'a, Result>;
    
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl From<Vec<Result>> for Results {
    fn from(results: Vec<Result>) -> Self {
        Self(results)
    }
}

impl AsRef<[Result]> for Results {
    fn as_ref(&self) -> &[Result] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_result_creation() {
        let result = Result::new(
            "123".to_string(),
            "Test Rule".to_string(),
            "Test Description".to_string(),
        );
        
        assert_eq!(result.id, "123");
        assert_eq!(result.title, "Test Rule");
        assert_eq!(result.description, "Test Description");
        assert!(result.tags.is_empty());
    }
    
    #[test]
    fn test_result_with_tags() {
        let result = Result::new(
            "123".to_string(),
            "Test Rule".to_string(),
            "Test Description".to_string(),
        )
        .with_tags(vec!["attack.discovery".to_string()]);
        
        assert_eq!(result.tags.len(), 1);
        assert_eq!(result.tags[0], "attack.discovery");
    }
    
    #[test]
    fn test_results_collection() {
        let mut results = Results::new();
        assert!(results.is_empty());
        
        results.push(Result::new(
            "123".to_string(),
            "Rule 1".to_string(),
            "Description 1".to_string(),
        ));
        
        results.push(Result::new(
            "456".to_string(),
            "Rule 2".to_string(),
            "Description 2".to_string(),
        ));
        
        assert_eq!(results.len(), 2);
        
        let ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
        assert_eq!(ids, vec!["123", "456"]);
    }
}