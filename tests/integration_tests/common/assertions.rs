//! Custom assertions for commit validation

use crate::common::git::Commit;

/// Extension trait for committing assertions
pub trait CommitAssertions {
    /// Assert commit has a specific type
    fn has_type(&self, expected: &str) -> &Self;
    
    /// Assert commit includes specific files
    fn includes_files(&self, files: &[&str]) -> &Self;
    
    /// Assert commit does NOT include specific files
    fn excludes_files(&self, files: &[&str]) -> &Self;
    
    /// Assert commit is auto-committable
    fn is_auto_committable(&self) -> &Self;
    
    /// Assert commit requires review
    fn requires_review(&self) -> &Self;
    
    /// Extract the commit type from message
    fn extract_type(&self) -> String;
    
    /// Assert commit message contains text
    fn has_message_containing(&self, text: &str) -> &Self;
    
    /// Start fluent assertions
    fn assert_that(&self) -> &Self;
}

impl CommitAssertions for Commit {
    fn assert_that(&self) -> &Self {
        self
    }
    fn has_type(&self, expected: &str) -> &Self {
        let actual_type = self.extract_type();
        assert_eq!(
            actual_type, expected,
            "Commit type mismatch. Expected '{}', got '{}'. Message: {}",
            expected, actual_type, self.message
        );
        self
    }
    
    fn includes_files(&self, files: &[&str]) -> &Self {
        for file in files {
            assert!(
                self.files.iter().any(|f| f == file),
                "Commit does not include file '{}'. Files: {:?}",
                file, self.files
            );
        }
        self
    }
    
    fn excludes_files(&self, files: &[&str]) -> &Self {
        for file in files {
            assert!(
                !self.files.iter().any(|f| f == file),
                "Commit unexpectedly includes file '{}'. Files: {:?}",
                file, self.files
            );
        }
        self
    }
    
    fn is_auto_committable(&self) -> &Self {
        // Check if commit type is in auto-committable list
        let auto_committable = ["docs", "chore", "dependency", "configuration", "style"];
        let commit_type = self.extract_type();
        assert!(
            auto_committable.contains(&commit_type.as_str()),
            "Commit type '{}' is not auto-committable. Message: {}",
            commit_type, self.message
        );
        self
    }
    
    fn requires_review(&self) -> &Self {
        // Check if commit type requires review
        let requires_review = ["feat", "feature", "fix", "bugfix", "refactor", "test"];
        let commit_type = self.extract_type();
        assert!(
            requires_review.contains(&commit_type.as_str()),
            "Commit type '{}' should require review. Message: {}",
            commit_type, self.message
        );
        self
    }
    
    fn extract_type(&self) -> String {
        // Parse conventional commit format: type(scope): message
        // or just type: message
        let msg = self.message.trim();
        
        // Find the first colon
        if let Some(colon_idx) = msg.find(':') {
            let prefix = &msg[..colon_idx];
            // Check if there's a scope (type(scope))
            if let Some(paren_idx) = prefix.find('(') {
                return prefix[..paren_idx].to_string();
            }
            return prefix.to_string();
        }
        
        // Fallback: try to infer from keywords
        let lower = msg.to_lowercase();
        if lower.starts_with("feat") { "feat" }
        else if lower.starts_with("fix") { "fix" }
        else if lower.starts_with("doc") { "docs" }
        else if lower.starts_with("refactor") { "refactor" }
        else if lower.starts_with("test") { "test" }
        else if lower.starts_with("chore") { "chore" }
        else if lower.starts_with("security") { "security" }
        else if lower.starts_with("config") { "chore" }
        else if lower.starts_with("dep") { "chore" }
        else { "unknown" }
        .to_string()
    }
    
    fn has_message_containing(&self, text: &str) -> &Self {
        assert!(
            self.message.contains(text),
            "Commit message does not contain '{}'. Message: {}",
            text, self.message
        );
        self
    }
}