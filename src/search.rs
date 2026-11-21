//! Everything-like AND / OR / exclusion / quoted phrase / wildcard search.
use regex::Regex;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

pub fn normalize(value: &str) -> String {
    value.nfkc().case_fold().collect()
}

#[derive(Default)]
pub struct Query(Vec<Vec<(Regex, bool)>>);
impl Query {
    pub fn new(input: &str) -> Self {
        let mut groups = vec![vec![]];
        let (mut token, mut inside, mut quoted, mut prefix_quoted) =
            (String::new(), false, false, false);
        let flush = |token: &mut String,
                     quoted: &mut bool,
                     prefix: &mut bool,
                     groups: &mut Vec<Vec<(Regex, bool)>>| {
            if !token.is_empty() {
                let exclude =
                    token.chars().count() > 1 && token.starts_with(['!', '-']) && !*prefix;
                let value = if exclude { &token[1..] } else { token.as_str() };
                let mut pattern = regex::escape(value);
                if !*quoted {
                    pattern = pattern.replace(r"\*", ".*").replace(r"\?", ".");
                }
                groups.last_mut().unwrap().push((
                    Regex::new(&format!("(?s){pattern}")).expect("escaped literal"),
                    exclude,
                ));
            }
            token.clear();
            *quoted = false;
            *prefix = false;
        };
        for c in normalize(input).chars() {
            if c == '"' {
                if token.is_empty() && !inside {
                    prefix_quoted = true;
                }
                inside = !inside;
                quoted = true;
            } else if !inside && (c.is_whitespace() || c == '|') {
                flush(&mut token, &mut quoted, &mut prefix_quoted, &mut groups);
                if c == '|' && !groups.last().unwrap().is_empty() {
                    groups.push(vec![]);
                }
            } else {
                token.push(c);
            }
        }
        flush(&mut token, &mut quoted, &mut prefix_quoted, &mut groups);
        groups.retain(|g| !g.is_empty());
        Self(groups)
    }
    pub fn matches(&self, text: &str) -> bool {
        let text = normalize(text);
        self.0.is_empty()
            || self.0.iter().any(|g| {
                g.iter()
                    .all(|(pattern, exclude)| pattern.is_match(&text) != *exclude)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composed_filters_and_partial_input() {
        for query in [
            "周杰伦 晴天 !live",
            "!live | July",
            "*.flac",
            "晴?",
            "| 周杰伦 |",
        ] {
            assert!(Query::new(query).matches("周杰伦 晴天 a.flac"), "{query}");
        }
        assert!(!Query::new("周杰伦 !live").matches("周杰伦 晴天 Live"));
        assert!(!Query::new("July !\"Love Theme\"").matches("July Love Theme"));
        assert!(Query::new("\"Love Theme").matches("July Love Theme"));
        assert!(!Query::new("\"Love Theme\"").matches("Love Another Theme"));
        assert!(Query::new("ＪＵＬＹ　theme").matches("July Love Theme"));
        assert!(Query::new("\"!Hello\"").matches("!Hello"));
        assert!(Query::new(r"C:\Music [live]").matches(r"C:\Music [live]"));
        assert!(Query::new("STRASSE").matches("Straße"));
    }
}
