use serde::{Deserialize, Deserializer, Serialize};

/// Maximum length for Result ID and title fields (256 characters)
const MAX_ID_TITLE_LENGTH: usize = 256;

/// Maximum length for Result description field (1 MB)
const MAX_DESCRIPTION_LENGTH: usize = 1024 * 1024;

/// Maximum number of tags allowed
const MAX_TAGS: usize = 100;

/// Maximum length for each tag
const MAX_TAG_LENGTH: usize = 128;

/// Validates string length during deserialization
fn validate_string<'de, D>(
    deserializer: D,
    max_len: usize,
    field_name: &str,
) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.len() > max_len {
        return Err(serde::de::Error::custom(format!(
            "{} exceeds maximum length of {} characters (got {})",
            field_name,
            max_len,
            s.len()
        )));
    }
    Ok(s)
}

/// Deserialize and validate ID field
fn deserialize_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    validate_string(deserializer, MAX_ID_TITLE_LENGTH, "id")
}

/// Deserialize and validate title field
fn deserialize_title<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    validate_string(deserializer, MAX_ID_TITLE_LENGTH, "title")
}

/// Deserialize and validate description field
fn deserialize_description<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    validate_string(deserializer, MAX_DESCRIPTION_LENGTH, "description")
}

/// Deserialize and validate tags
fn deserialize_tags<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let tags = Vec::<String>::deserialize(deserializer)?;

    if tags.len() > MAX_TAGS {
        return Err(serde::de::Error::custom(format!(
            "Too many tags: maximum {} allowed (got {})",
            MAX_TAGS,
            tags.len()
        )));
    }

    for (i, tag) in tags.iter().enumerate() {
        if tag.len() > MAX_TAG_LENGTH {
            return Err(serde::de::Error::custom(format!(
                "Tag at index {} exceeds maximum length of {} characters (got {})",
                i,
                MAX_TAG_LENGTH,
                tag.len()
            )));
        }
    }

    Ok(tags)
}

/// Result of a positive Sigma rule match
///
/// This struct represents a successful match of a Sigma rule against an event.
/// It contains the rule metadata and any associated tags.
///
/// # Examples
///
/// ```
/// use sigma::result::Result;
///
/// let result = Result::new(
///     "rule-001".to_string(),
///     "Suspicious Process Creation".to_string(),
///     "Detects suspicious process creation patterns".to_string(),
/// );
///
/// // Add tags to categorize the detection
/// let result = result.with_tags(vec![
///     "attack.execution".to_string(),
///     "attack.t1059".to_string(),
/// ]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Result {
    /// Tags associated with this detection (e.g., MITRE ATT&CK tags)
    #[serde(default, deserialize_with = "deserialize_tags")]
    pub tags: Vec<String>,

    /// Unique identifier for the rule that matched
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,

    /// Human-readable title of the rule
    #[serde(deserialize_with = "deserialize_title")]
    pub title: String,

    /// Detailed description of what the rule detects
    #[serde(deserialize_with = "deserialize_description")]
    pub description: String,
}

impl Result {
    /// Creates a new Result with the specified rule metadata
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the rule
    /// * `title` - Human-readable title
    /// * `description` - Detailed description of the detection
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::Result;
    ///
    /// let result = Result::new(
    ///     "proc-001".to_string(),
    ///     "Malicious PowerShell".to_string(),
    ///     "Detects encoded PowerShell commands".to_string(),
    /// );
    /// ```
    pub fn new(id: String, title: String, description: String) -> Self {
        Self {
            tags: Vec::new(),
            id,
            title,
            description,
        }
    }

    /// Adds tags to this Result
    ///
    /// This method consumes self and returns a new Result with the specified tags.
    /// Useful for builder-pattern construction.
    ///
    /// # Arguments
    ///
    /// * `tags` - Vector of tag strings (e.g., MITRE ATT&CK techniques)
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::Result;
    ///
    /// let result = Result::new(
    ///     "rule-001".to_string(),
    ///     "Suspicious Activity".to_string(),
    ///     "Detects suspicious behavior".to_string(),
    /// )
    /// .with_tags(vec!["attack.defense_evasion".to_string()]);
    /// ```
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Collection of Results from multiple rule matches
///
/// This struct represents the outcome when a single event matches multiple Sigma rules.
/// It provides a type-safe wrapper around a vector of Results with convenient methods
/// for manipulation and iteration.
///
/// # Examples
///
/// ```
/// use sigma::result::{Result, Results};
///
/// let mut results = Results::new();
///
/// // Add results as rules match
/// results.push(Result::new(
///     "rule-1".to_string(),
///     "First Detection".to_string(),
///     "Description 1".to_string(),
/// ));
///
/// results.push(Result::new(
///     "rule-2".to_string(),
///     "Second Detection".to_string(),
///     "Description 2".to_string(),
/// ));
///
/// // Check if any rules matched
/// if !results.is_empty() {
///     println!("Found {} matches", results.len());
///     
///     // Iterate over results
///     for result in &results {
///         println!("Matched rule: {}", result.title);
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Results(Vec<Result>);

impl Results {
    /// Creates a new empty Results collection
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::Results;
    ///
    /// let results = Results::new();
    /// assert!(results.is_empty());
    /// ```
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Adds a Result to the collection
    ///
    /// # Arguments
    ///
    /// * `result` - The Result to add to this collection
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::{Result, Results};
    ///
    /// let mut results = Results::new();
    /// results.push(Result::new(
    ///     "test-rule".to_string(),
    ///     "Test Rule".to_string(),
    ///     "Test Description".to_string(),
    /// ));
    /// ```
    pub fn push(&mut self, result: Result) {
        self.0.push(result);
    }

    /// Returns true if the collection contains no results
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::Results;
    ///
    /// let results = Results::new();
    /// assert!(results.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of results in the collection
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::{Result, Results};
    ///
    /// let mut results = Results::new();
    /// assert_eq!(results.len(), 0);
    ///
    /// results.push(Result::new(
    ///     "rule-1".to_string(),
    ///     "Rule 1".to_string(),
    ///     "Description".to_string(),
    /// ));
    /// assert_eq!(results.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns an iterator over the results
    ///
    /// # Examples
    ///
    /// ```
    /// use sigma::result::{Result, Results};
    ///
    /// let mut results = Results::new();
    /// results.push(Result::new(
    ///     "rule-1".to_string(),
    ///     "Rule 1".to_string(),
    ///     "Description 1".to_string(),
    /// ));
    ///
    /// for result in results.iter() {
    ///     println!("Rule ID: {}", result.id);
    /// }
    /// ```
    pub fn iter(&self) -> std::slice::Iter<'_, Result> {
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

        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["123", "456"]);
    }

    #[test]
    fn test_deserialization_validation() {
        // Valid result should deserialize successfully
        let valid_json = r#"{
            "id": "test-id",
            "title": "Test Title",
            "description": "Test Description",
            "tags": ["tag1", "tag2"]
        }"#;

        let result: Result = serde_json::from_str(valid_json).unwrap();
        assert_eq!(result.id, "test-id");
        assert_eq!(result.tags.len(), 2);
    }

    #[test]
    fn test_deserialization_id_too_long() {
        let long_id = "a".repeat(MAX_ID_TITLE_LENGTH + 1);
        let invalid_json = format!(
            r#"{{
                "id": "{}",
                "title": "Test",
                "description": "Test"
            }}"#,
            long_id
        );

        let result: std::result::Result<Result, _> = serde_json::from_str(&invalid_json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("id exceeds maximum length"));
    }

    #[test]
    fn test_deserialization_title_too_long() {
        let long_title = "a".repeat(MAX_ID_TITLE_LENGTH + 1);
        let invalid_json = format!(
            r#"{{
                "id": "test",
                "title": "{}",
                "description": "Test"
            }}"#,
            long_title
        );

        let result: std::result::Result<Result, _> = serde_json::from_str(&invalid_json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("title exceeds maximum length"));
    }

    #[test]
    fn test_deserialization_too_many_tags() {
        let tags: Vec<String> = (0..MAX_TAGS + 1).map(|i| format!("tag{}", i)).collect();
        let tags_json = serde_json::to_string(&tags).unwrap();

        let invalid_json = format!(
            r#"{{
                "id": "test",
                "title": "Test",
                "description": "Test",
                "tags": {}
            }}"#,
            tags_json
        );

        let result: std::result::Result<Result, _> = serde_json::from_str(&invalid_json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Too many tags"));
    }

    #[test]
    fn test_deserialization_tag_too_long() {
        let long_tag = "a".repeat(MAX_TAG_LENGTH + 1);
        let invalid_json = format!(
            r#"{{
                "id": "test",
                "title": "Test",
                "description": "Test",
                "tags": ["{}"]
            }}"#,
            long_tag
        );

        let result: std::result::Result<Result, _> = serde_json::from_str(&invalid_json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Tag at index 0 exceeds maximum length"));
    }

    #[test]
    fn test_results_iteration() {
        let results = Results::from(vec![
            Result::new("1".to_string(), "R1".to_string(), "D1".to_string()),
            Result::new("2".to_string(), "R2".to_string(), "D2".to_string()),
        ]);

        // Test iter()
        let count = results.iter().count();
        assert_eq!(count, 2);

        // Test into_iter() for &Results
        let count = (&results).into_iter().count();
        assert_eq!(count, 2);

        // Test into_iter() for Results
        let count = results.into_iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_results_as_ref() {
        let results = Results::from(vec![Result::new(
            "1".to_string(),
            "R1".to_string(),
            "D1".to_string(),
        )]);

        let slice: &[Result] = results.as_ref();
        assert_eq!(slice.len(), 1);
    }
}
