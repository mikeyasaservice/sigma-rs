use serde::{Deserialize, Serialize};

/// Tags contains a metadata list for tying positive matches together with other threat intel sources
/// For example, for attaching MITRE ATT&CK tactics or techniques to the event
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Tags(pub Vec<String>);

impl Tags {
    /// Create new empty tags collection
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create tags from a vector of strings
    pub fn from_vec(tags: Vec<String>) -> Self {
        Self(tags)
    }

    /// Check if all provided tags are present in this collection
    pub fn has_all(&self, tags: &[String]) -> bool {
        tags.iter().all(|tag| self.0.contains(tag))
    }

    /// Check if any of the provided tags are present in this collection
    pub fn has_any(&self, tags: &[String]) -> bool {
        tags.iter().any(|tag| self.0.contains(tag))
    }

    /// Add a tag to the collection
    pub fn add(&mut self, tag: String) {
        if !self.0.contains(&tag) {
            self.0.push(tag);
        }
    }

    /// Get underlying vector reference
    pub fn as_vec(&self) -> &Vec<String> {
        &self.0
    }

    /// Get mutable reference to underlying vector
    pub fn as_mut_vec(&mut self) -> &mut Vec<String> {
        &mut self.0
    }
}

impl IntoIterator for Tags {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Tags {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl From<Vec<String>> for Tags {
    fn from(tags: Vec<String>) -> Self {
        Self(tags)
    }
}

impl From<&[&str]> for Tags {
    fn from(tags: &[&str]) -> Self {
        Self(tags.iter().map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tags_has_all() {
        let tags = Tags::from(&["attack.discovery", "attack.t1069.001"][..]);

        assert!(tags.has_all(&["attack.discovery".to_string()]));
        assert!(tags.has_all(&[
            "attack.discovery".to_string(),
            "attack.t1069.001".to_string()
        ]));
        assert!(!tags.has_all(&[
            "attack.discovery".to_string(),
            "attack.execution".to_string()
        ]));
    }

    #[test]
    fn test_tags_has_any() {
        let tags = Tags::from(&["attack.discovery", "attack.t1069.001"][..]);

        assert!(tags.has_any(&["attack.discovery".to_string()]));
        assert!(tags.has_any(&[
            "attack.execution".to_string(),
            "attack.discovery".to_string()
        ]));
        assert!(!tags.has_any(&["attack.execution".to_string()]));
    }

    #[test]
    fn test_tags_add() {
        let mut tags = Tags::new();
        tags.add("attack.discovery".to_string());
        tags.add("attack.discovery".to_string()); // Duplicate should not be added
        tags.add("attack.execution".to_string());

        assert_eq!(tags.as_vec().len(), 2);
        assert!(tags.has_all(&[
            "attack.discovery".to_string(),
            "attack.execution".to_string()
        ]));
    }
}
