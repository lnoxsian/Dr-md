use regex::{Regex, RegexBuilder};
use std::fmt;

const MAX_REGEX_SIZE_LIMIT: usize = 1024 * 1024; // 1 MB limit to prevent ReDoS

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    UnknownOperator(String),
    InvalidBooleanValue { operator: String, value: String },
    UnclosedQuote,
    InvalidRegex(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::UnknownOperator(op) => write!(
                f,
                "Unknown search operator '{op}:'. Valid operators: path:, file:, tag:, line:, case:, word:, regex:"
            ),
            QueryError::InvalidBooleanValue { operator, value } => write!(
                f,
                "Invalid value '{value}' for '{operator}:'. Expected 'true' or 'false'."
            ),
            QueryError::UnclosedQuote => write!(f, "Unclosed quote in search query."),
            QueryError::InvalidRegex(err) => write!(f, "Invalid regular expression: {err}"),
        }
    }
}

impl std::error::Error for QueryError {}

/// Parsed search query with filters and configuration.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub raw: String,
    pub path_filter: Option<String>,
    pub file_filter: Option<String>,
    pub tag_filters: Vec<String>,
    pub line_filter: Option<String>,
    pub terms: Vec<String>,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub is_regex: bool,
    pub compiled_regex: Option<Regex>,
}

impl SearchQuery {
    /// Parse query string with default UI toggle settings.
    pub fn parse(
        input: &str,
        ui_case_sensitive: bool,
        ui_whole_word: bool,
        ui_is_regex: bool,
    ) -> Result<Self, QueryError> {
        let mut path_filter = None;
        let mut file_filter = None;
        let mut tag_filters = Vec::new();
        let mut line_filter = None;
        let mut terms = Vec::new();

        let mut explicit_regex_operator = false;
        let mut case_sensitive = ui_case_sensitive;
        let mut whole_word = ui_whole_word;
        let mut is_regex = ui_is_regex;

        let tokens = tokenize_query(input)?;

        for token in tokens {
            if let Some(colon_pos) = token.find(':') {
                let prefix = &token[..colon_pos];
                let value = &token[colon_pos + 1..];

                // Check for URL schemes: http:// or https://
                if prefix.eq_ignore_ascii_case("http") || prefix.eq_ignore_ascii_case("https") {
                    terms.push(token);
                    continue;
                }

                // Check known operators
                match prefix.to_ascii_lowercase().as_str() {
                    "path" => {
                        path_filter = Some(value.to_string());
                    }
                    "file" => {
                        file_filter = Some(value.to_string());
                    }
                    "tag" => {
                        let normalized = value.trim_start_matches('#').to_string();
                        if !normalized.is_empty() {
                            tag_filters.push(normalized);
                        }
                    }
                    "line" => {
                        line_filter = Some(value.to_string());
                    }
                    "case" => {
                        case_sensitive = parse_bool(prefix, value)?;
                    }
                    "word" => {
                        whole_word = parse_bool(prefix, value)?;
                    }
                    "regex" => {
                        is_regex = parse_bool(prefix, value)?;
                        explicit_regex_operator = true;
                    }
                    _ => {
                        // If it's alphanumeric, treat as unknown operator
                        if prefix.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            return Err(QueryError::UnknownOperator(prefix.to_string()));
                        } else {
                            terms.push(token);
                        }
                    }
                }
            } else {
                terms.push(token);
            }
        }

        // Clean up slash-wrapped tokens like "/pattern/" into "pattern"
        for term in &mut terms {
            if term.starts_with('/') && term.ends_with('/') && term.len() >= 2 {
                *term = term[1..term.len() - 1].to_string();
            }
        }

        // If regex was not explicitly configured via regex: operator or UI,
        // automatically detect if any term or line filter contains regex syntax.
        if !explicit_regex_operator && !is_regex {
            let terms_have_regex = terms.iter().any(|t| is_likely_regex(t));
            let line_has_regex = line_filter
                .as_ref()
                .map(|lf| is_likely_regex(lf))
                .unwrap_or(false);
            if terms_have_regex || line_has_regex {
                is_regex = true;
            }
        }

        let compiled_regex = if is_regex && (!terms.is_empty() || line_filter.is_some()) {
            let pattern = if let Some(ref lf) = line_filter {
                lf.clone()
            } else if terms.len() == 1 {
                terms[0].clone()
            } else {
                // If multiple terms in regex mode, match line containing any or all
                terms.join(".*")
            };

            let mut builder = RegexBuilder::new(&pattern);
            builder.case_insensitive(!case_sensitive);
            builder.size_limit(MAX_REGEX_SIZE_LIMIT);
            match builder.build() {
                Ok(re) => Some(re),
                Err(e) => {
                    if explicit_regex_operator || ui_is_regex {
                        return Err(QueryError::InvalidRegex(e.to_string()));
                    } else {
                        // Autodetected regex failed to compile (e.g. typing an unclosed bracket).
                        // Fall back cleanly to plain text search!
                        is_regex = false;
                        None
                    }
                }
            }
        } else {
            None
        };

        Ok(Self {
            raw: input.to_string(),
            path_filter,
            file_filter,
            tag_filters,
            line_filter,
            terms,
            case_sensitive,
            whole_word,
            is_regex,
            compiled_regex,
        })
    }

    /// Whether this query is completely empty (no terms, no filters).
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
            && self.path_filter.is_none()
            && self.file_filter.is_none()
            && self.tag_filters.is_empty()
            && self.line_filter.is_none()
    }
}

fn parse_bool(op: &str, val: &str) -> Result<bool, QueryError> {
    match val.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(QueryError::InvalidBooleanValue {
            operator: op.to_string(),
            value: val.to_string(),
        }),
    }
}

/// Heuristic detection of regular expression syntax.
pub fn is_likely_regex(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }

    // Enclosed in slashes, e.g. /pattern/
    if s.len() >= 2 && s.starts_with('/') && s.ends_with('/') {
        return true;
    }

    // Common regex character classes / escapes
    if s.contains(r"\d")
        || s.contains(r"\D")
        || s.contains(r"\w")
        || s.contains(r"\W")
        || s.contains(r"\s")
        || s.contains(r"\S")
        || s.contains(r"\b")
        || s.contains(r"\B")
        || s.contains(r"\p")
        || s.contains(r"\P")
    {
        return true;
    }

    // Wildcards with quantifiers: .*, .+, .?, .{
    if s.contains(".*") || s.contains(".+") || s.contains(".?") || s.contains(".{") {
        return true;
    }

    // Alternation: | (e.g. foo|bar, (a|b))
    if s.contains('|') {
        return true;
    }

    // Grouping: (...) with non-empty inside
    if let (Some(open), Some(close)) = (s.find('('), s.rfind(')'))
        && open < close
    {
        return true;
    }

    // Character class range or negation: [a-z], [0-9], [^...]
    if let (Some(open), Some(close)) = (s.find('['), s.rfind(']'))
        && open < close
    {
        let inner = &s[open + 1..close];
        if inner.contains('-') || inner.starts_with('^') || inner.contains('\\') {
            return true;
        }
    }

    // Line anchors: ^ at start, $ at end
    if s.starts_with('^') || (s.ends_with('$') && !s.ends_with(r"\$")) {
        return true;
    }

    // Quantifier braces: {n}, {n,}, {n,m}
    if let (Some(open), Some(close)) = (s.find('{'), s.rfind('}'))
        && open < close
    {
        let inner = &s[open + 1..close];
        if inner.chars().all(|c| c.is_ascii_digit() || c == ',') && !inner.is_empty() {
            return true;
        }
    }

    // Quantifier following a closing paren or bracket: )+, )*, )?, ]+, ]*, ]?
    if s.contains(")+")
        || s.contains(")*")
        || s.contains(")?")
        || s.contains("]+")
        || s.contains("]*")
        || s.contains("]?")
    {
        return true;
    }

    false
}

/// Tokenizes a query string, preserving quoted phrases like `"exact phrase"`, `/regex literal/`, or `path:"my notes"`.
fn tokenize_query(input: &str) -> Result<Vec<String>, QueryError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut in_regex_slashes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            '/' if !in_quotes => {
                if current.is_empty() && !in_regex_slashes {
                    in_regex_slashes = true;
                    current.push('/');
                } else if in_regex_slashes {
                    in_regex_slashes = false;
                    current.push('/');
                } else {
                    current.push('/');
                }
            }
            ' ' | '\t' | '\r' | '\n' if !in_quotes && !in_regex_slashes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            '\\' => {
                if in_quotes || in_regex_slashes {
                    if let Some(&next_c) = chars.peek() {
                        if next_c == '"' || next_c == '\\' || next_c == '/' {
                            current.push(next_c);
                            chars.next();
                        } else {
                            current.push('\\');
                        }
                    } else {
                        current.push('\\');
                    }
                } else {
                    current.push('\\');
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if in_quotes {
        return Err(QueryError::UnclosedQuote);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_terms() {
        let q = SearchQuery::parse("hello world", false, false, false).unwrap();
        assert_eq!(q.terms, vec!["hello", "world"]);
        assert!(!q.case_sensitive);
        assert!(!q.whole_word);
        assert!(!q.is_regex);
    }

    #[test]
    fn test_quoted_phrases() {
        let q = SearchQuery::parse(r#""exact match" another"#, false, false, false).unwrap();
        assert_eq!(q.terms, vec!["exact match", "another"]);
    }

    #[test]
    fn test_operators() {
        let q = SearchQuery::parse(
            "path:notes/ file:daily.md tag:#project line:term case:true word:true regex:false",
            false,
            false,
            false,
        )
        .unwrap();

        assert_eq!(q.path_filter, Some("notes/".to_string()));
        assert_eq!(q.file_filter, Some("daily.md".to_string()));
        assert_eq!(q.tag_filters, vec!["project".to_string()]);
        assert_eq!(q.line_filter, Some("term".to_string()));
        assert!(q.case_sensitive);
        assert!(q.whole_word);
        assert!(!q.is_regex);
    }

    #[test]
    fn test_unknown_operator_error() {
        let res = SearchQuery::parse("foo:bar search", false, false, false);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            QueryError::UnknownOperator("foo".to_string())
        );
    }

    #[test]
    fn test_invalid_boolean_operator() {
        let res = SearchQuery::parse("case:maybe search", false, false, false);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            QueryError::InvalidBooleanValue {
                operator: "case".to_string(),
                value: "maybe".to_string()
            }
        );
    }

    #[test]
    fn test_unclosed_quote_error() {
        let res = SearchQuery::parse(r#""unclosed quote"#, false, false, false);
        assert_eq!(res.unwrap_err(), QueryError::UnclosedQuote);
    }

    #[test]
    fn test_regex_compilation_and_error() {
        let q = SearchQuery::parse("fn\\s+\\w+", false, false, true).unwrap();
        assert!(q.compiled_regex.is_some());

        let invalid = SearchQuery::parse("(unclosed_group", false, false, true);
        assert!(invalid.is_err());
        if let Err(QueryError::InvalidRegex(msg)) = invalid {
            assert!(msg.contains("unclosed"));
        } else {
            panic!("Expected InvalidRegex error");
        }
    }

    #[test]
    fn test_url_not_treated_as_operator() {
        let q = SearchQuery::parse("https://example.com/api", false, false, false).unwrap();
        assert_eq!(q.terms, vec!["https://example.com/api"]);
    }

    #[test]
    fn test_regex_autodetection() {
        // Plain query: not regex
        let q1 = SearchQuery::parse("rust programming", false, false, false).unwrap();
        assert!(!q1.is_regex);
        assert!(q1.compiled_regex.is_none());

        // Regex pattern: autodetected without ui_is_regex
        let q2 = SearchQuery::parse(r"item-\d+", false, false, false).unwrap();
        assert!(q2.is_regex);
        assert!(q2.compiled_regex.is_some());

        // Alternation
        let q3 = SearchQuery::parse("cat|dog", false, false, false).unwrap();
        assert!(q3.is_regex);
        assert!(q3.compiled_regex.is_some());

        // Slashes /pattern/ with spaces
        let q4 = SearchQuery::parse("/exact.*pat with spaces/", false, false, false).unwrap();
        assert!(q4.is_regex);
        assert!(q4.compiled_regex.is_some());

        // Autodetection with syntax error falls back to plain text instead of crashing
        let q5 = SearchQuery::parse("foo(bar", false, false, false).unwrap();
        assert!(!q5.is_regex);
        assert!(q5.compiled_regex.is_none());

        // Explicit regex:true still returns error on syntax failure
        let q6 = SearchQuery::parse("regex:true foo(bar", false, false, false);
        assert!(q6.is_err());
    }
}
