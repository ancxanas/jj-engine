#[must_use]
pub fn extract_evidence_block(message: &str) -> Option<&str> {
    let start = message.find("AVCS-EVIDENCE:")?;
    Some(&message[start..])
}

#[must_use]
pub fn extract_pattern(evidence_block: &str) -> Option<&str> {
    for line in evidence_block.lines() {
        let trimmed = line.trim();
        if let Some(pattern) = trimmed.strip_prefix("Pattern: ") {
            return Some(pattern);
        }
    }
    None
}

#[must_use]
pub fn extract_rule(evidence_block: &str) -> Option<&str> {
    for line in evidence_block.lines() {
        let trimmed = line.trim();
        if let Some(rule) = trimmed.strip_prefix("Rule: ") {
            return Some(rule);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "docs: update README\n\nAVCS-EVIDENCE:\n  Pattern: Documentation\n  Files:\n    - README.md\n  Rule: documentation-change\n  Engine-Version: 0.1.0\n";

    #[test]
    fn extracts_evidence_block() -> anyhow::Result<()> {
        let block = extract_evidence_block(SAMPLE)
            .ok_or_else(|| anyhow::anyhow!("missing evidence block"))?;
        assert!(block.starts_with("AVCS-EVIDENCE:"));
        Ok(())
    }

    #[test]
    fn extracts_pattern() -> anyhow::Result<()> {
        let block = extract_evidence_block(SAMPLE)
            .ok_or_else(|| anyhow::anyhow!("missing evidence block"))?;
        assert_eq!(extract_pattern(block), Some("Documentation"));
        Ok(())
    }

    #[test]
    fn extracts_rule() -> anyhow::Result<()> {
        let block = extract_evidence_block(SAMPLE)
            .ok_or_else(|| anyhow::anyhow!("missing evidence block"))?;
        assert_eq!(extract_rule(block), Some("documentation-change"));
        Ok(())
    }

    #[test]
    fn returns_none_for_no_evidence() {
        assert!(extract_evidence_block("just a plain commit").is_none());
    }
}
