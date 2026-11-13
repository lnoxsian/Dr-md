use std::path::Path;

#[allow(dead_code)]
pub fn preprocess_wiki_links(content: &str) -> String {
    preprocess_wiki_links_with_context(content, None, None)
}

#[allow(dead_code)]
pub fn preprocess_wiki_links_with_context(
    content: &str,
    current_file: Option<&Path>,
    vault_root: Option<&Path>,
) -> String {
    let mut call_stack = Vec::new();
    super::wikilink::preprocess_markdown_with_embeds(
        content,
        current_file,
        vault_root,
        &mut call_stack,
        0,
    )
    .text
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
