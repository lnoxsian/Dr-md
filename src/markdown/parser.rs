pub fn preprocess_wiki_links(content: &str) -> String {
    if !content.contains("[[") {
        return content.to_string();
    }

    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek() == Some(&'[') {
            chars.next(); // consume second '['
            let mut link = String::new();
            let mut found = false;
            while let Some(&nc) = chars.peek() {
                if nc == ']' {
                    chars.next();
                    if chars.peek() == Some(&']') {
                        chars.next(); // consume second ']'
                        found = true;
                        break;
                    } else {
                        link.push(']');
                    }
                } else {
                    link.push(chars.next().unwrap());
                }
            }
            if found {
                result.push('[');
                if let Some(pipe_pos) = link.find('|') {
                    let path = &link[..pipe_pos];
                    let label = &link[pipe_pos + 1..];
                    result.push_str(label);
                    result.push_str("](");
                    result.push_str(path);
                } else {
                    result.push_str(&link);
                    result.push_str("](");
                    result.push_str(&link);
                }
                result.push_str(".md)");
            } else {
                result.push_str("[[");
                result.push_str(&link);
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_wiki_links_complete() {
        assert_eq!(preprocess_wiki_links("[[hello]]"), "[hello](hello.md)");
        assert_eq!(
            preprocess_wiki_links("[[hello|label]]"),
            "[label](hello.md)"
        );
    }

    #[test]
    fn test_preprocess_wiki_links_incomplete() {
        assert_eq!(preprocess_wiki_links("[[hello"), "[[hello");
        assert_eq!(preprocess_wiki_links("[[hello]"), "[[hello]");
        assert_eq!(preprocess_wiki_links("hello [ world"), "hello [ world");
    }
}
