//! Serialization follows src/lib/commit-message.ts in the original VS Code extension.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitMessage {
    pub kind: String,
    pub scope: String,
    pub gitmoji: String,
    pub subject: String,
    pub body: String,
    pub footer: String,
    pub ci: bool,
    pub tag: String,
}

impl CommitMessage {
    pub fn normalize(&mut self) {
        self.kind = self.kind.trim().to_string();
        self.scope = self.scope.trim().to_string();
        self.gitmoji = self.gitmoji.trim().to_string();
        self.subject = self.subject.trim().to_string();
        self.body = self.body.trim().to_string();
        self.footer = self.footer.trim().to_string();
        self.tag = self.tag.trim().to_string();
    }

    pub fn subject_line(&self) -> String {
        match (self.gitmoji.is_empty(), self.subject.is_empty()) {
            (true, _) => self.subject.clone(),
            (_, true) => self.gitmoji.clone(),
            (false, false) => format!("{} {}", self.gitmoji, self.subject),
        }
    }

    pub fn header(&self) -> String {
        let mut text = self.kind.clone();
        if !self.scope.is_empty() {
            text.push_str(&format!("({})", self.scope));
        }
        text.push_str(": ");
        text.push_str(&self.subject_line());
        if !self.tag.is_empty() {
            text.push(' ');
            text.push_str(&self.tag);
        }
        if self.ci {
            text.push_str(" [skip ci]");
        }
        text
    }

    pub fn serialize(&self) -> String {
        let mut text = self.header();
        if !self.body.is_empty() {
            text.push_str("\n\n");
            text.push_str(&self.body);
        }
        if !self.footer.is_empty() {
            text.push_str("\n\n");
            text.push_str(&self.footer);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_message_regression() {
        let mut m = CommitMessage {
            kind: "  feat  ".into(),
            scope: "  api ".into(),
            gitmoji: " :sparkles: ".into(),
            subject: " add login ".into(),
            body: " rationale ".into(),
            footer: " BREAKING CHANGE: endpoint ".into(),
            ci: true,
            tag: " [release] ".into(),
        };
        m.normalize();
        assert_eq!(m.header(), "feat(api): :sparkles: add login [release] [skip ci]");
        assert_eq!(m.serialize(), "feat(api): :sparkles: add login [release] [skip ci]\n\nrationale\n\nBREAKING CHANGE: endpoint");
    }

    #[test]
    fn header_with_empty_subject_matches_original() {
        assert_eq!(CommitMessage { kind: "feat".into(), ..Default::default() }.header(), "feat: ");
    }

    #[test]
    fn skips_blank_sections() {
        let m = CommitMessage { kind: "fix".into(), subject: "handle spaces".into(), footer: "Refs: #3".into(), ..Default::default() };
        assert_eq!(m.serialize(), "fix: handle spaces\n\nRefs: #3");
    }
}
