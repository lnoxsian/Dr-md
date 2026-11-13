use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Represents a parsed wikilink or embed tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedWikiLink {
    /// True if the link was prefixed with '!' (embed), e.g. ![[note]]
    pub is_embed: bool,
    /// Target path/filename (e.g. "note", "note.md", "folder/note", or "" for current file anchor)
    pub target: String,
    /// Heading anchor without '#' (e.g. "Heading Name" or "heading-1")
    pub heading: Option<String>,
    /// Block reference without '^' (e.g. "block-id")
    pub block_id: Option<String>,
    /// Display alias (e.g. "custom alias" in [[note|custom alias]])
    pub alias: Option<String>,
    /// Full raw text of the link including delimiters (e.g. "![[note#H|A]]")
    pub raw: String,
    /// Byte span in the source string
    pub span: std::ops::Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Audio,
    Video,
    Pdf,
    Other,
}

pub fn detect_media_type(path: &Path) -> Option<MediaType> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "tiff" | "ico" => {
            Some(MediaType::Image)
        }
        "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac" => Some(MediaType::Audio),
        "mp4" | "webm" | "ogv" | "mov" | "mkv" => Some(MediaType::Video),
        "pdf" => Some(MediaType::Pdf),
        "md" | "markdown" => None,
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLinkTarget {
    CurrentDocument,
    MarkdownFile {
        path: PathBuf,
        exists: bool,
    },
    MediaOrFile {
        path: PathBuf,
        exists: bool,
        media_type: MediaType,
    },
    Directory {
        path: PathBuf,
        exists: bool,
    },
}

impl ResolvedLinkTarget {
    #[allow(dead_code)]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::MarkdownFile { path, .. }
            | Self::MediaOrFile { path, .. }
            | Self::Directory { path, .. } => Some(path),
            Self::CurrentDocument => None,
        }
    }

    #[allow(dead_code)]
    pub fn exists(&self) -> bool {
        match self {
            Self::MarkdownFile { exists, .. }
            | Self::MediaOrFile { exists, .. }
            | Self::Directory { exists, .. } => *exists,
            Self::CurrentDocument => true,
        }
    }
}

/// Structured tokenizer that parses all wikilinks in a Markdown document.
/// Accurately skips code blocks (fenced with ``` or ~~~) and inline code spans (`...`).
pub fn parse_wiki_links(content: &str) -> Vec<ParsedWikiLink> {
    let mut links = Vec::new();
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    let mut in_fence = false;
    let mut fence_char = b'`';
    let mut fence_len = 0;

    let mut in_inline_code = false;
    let mut inline_code_ticks = 0;

    while i < len {
        // Handle code blocks and inline code
        if !in_fence && !in_inline_code {
            // Check for fence start at line beginning
            let is_line_start = i == 0 || bytes[i - 1] == b'\n';
            if is_line_start && (bytes[i] == b'`' || bytes[i] == b'~') {
                let fc = bytes[i];
                let count = bytes[i..].iter().take_while(|&&b| b == fc).count();
                if count >= 3 {
                    in_fence = true;
                    fence_char = fc;
                    fence_len = count;
                    i += count;
                    continue;
                }
            }

            // Check for inline code backticks
            if bytes[i] == b'`' {
                let count = bytes[i..].iter().take_while(|&&b| b == b'`').count();
                in_inline_code = true;
                inline_code_ticks = count;
                i += count;
                continue;
            }
        } else if in_fence {
            let is_line_start = i == 0 || bytes[i - 1] == b'\n';
            if is_line_start && bytes[i] == fence_char {
                let count = bytes[i..].iter().take_while(|&&b| b == fence_char).count();
                if count >= fence_len {
                    in_fence = false;
                    i += count;
                    continue;
                }
            }
            i += 1;
            continue;
        } else if in_inline_code {
            if bytes[i] == b'`' {
                let count = bytes[i..].iter().take_while(|&&b| b == b'`').count();
                if count == inline_code_ticks {
                    in_inline_code = false;
                    i += count;
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // Look for '![[' or '[['
        let is_embed = bytes[i] == b'!' && i + 2 < len && &bytes[i + 1..i + 3] == b"[[";
        let is_link = !is_embed && i + 1 < len && &bytes[i..i + 2] == b"[[";

        if is_embed || is_link {
            let start = i;
            let open_bracket_pos = if is_embed { i + 1 } else { i };
            let inner_start = open_bracket_pos + 2;

            if let Some(rel_close) = content[inner_start..].find("]]") {
                let inner_end = inner_start + rel_close;
                let end = inner_end + 2;
                let inner = &content[inner_start..inner_end];

                // Parse target and alias separated by '|'
                let (target_part, alias) = if let Some(pipe_pos) = inner.find('|') {
                    (
                        &inner[..pipe_pos],
                        Some(inner[pipe_pos + 1..].trim().to_string()),
                    )
                } else {
                    (inner, None)
                };

                let target_trimmed = target_part.trim();

                // Parse target, heading, block_id
                let (target, heading, block_id) = parse_target_expression(target_trimmed);

                links.push(ParsedWikiLink {
                    is_embed,
                    target,
                    heading,
                    block_id,
                    alias,
                    raw: content[start..end].to_string(),
                    span: start..end,
                });

                i = end;
                continue;
            }
        }

        i += 1;
    }

    links
}

fn parse_target_expression(expr: &str) -> (String, Option<String>, Option<String>) {
    if expr.is_empty() {
        return (String::new(), None, None);
    }

    // Check for block anchor starting with '#^' or '^'
    if let Some(hash_block_pos) = expr.find("#^") {
        let target = expr[..hash_block_pos].trim().to_string();
        let block_id = expr[hash_block_pos + 2..].trim().to_string();
        return (
            target,
            None,
            if block_id.is_empty() {
                None
            } else {
                Some(block_id)
            },
        );
    }

    if let Some(block_pos) = expr.find('^') {
        let target = expr[..block_pos].trim().to_string();
        let block_id = expr[block_pos + 1..].trim().to_string();
        return (
            target,
            None,
            if block_id.is_empty() {
                None
            } else {
                Some(block_id)
            },
        );
    }

    // Check for heading anchor '#', which might also contain a block reference e.g. '#heading^block'
    if let Some(hash_pos) = expr.find('#') {
        let target = expr[..hash_pos].trim().to_string();
        let rest = &expr[hash_pos + 1..];

        if let Some(block_pos) = rest.find('^') {
            let heading = rest[..block_pos].trim().to_string();
            let block_id = rest[block_pos + 1..].trim().to_string();
            return (
                target,
                if heading.is_empty() {
                    None
                } else {
                    Some(heading)
                },
                if block_id.is_empty() {
                    None
                } else {
                    Some(block_id)
                },
            );
        }

        let heading = rest.trim().to_string();
        return (
            target,
            if heading.is_empty() {
                None
            } else {
                Some(heading)
            },
            None,
        );
    }

    (expr.trim().to_string(), None, None)
}

/// Normalizes path components without touching the filesystem.
pub fn clean_path(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => match clean.components().next_back() {
                Some(Component::Normal(_)) => {
                    clean.pop();
                }
                Some(Component::ParentDir) | None => {
                    clean.push(Component::ParentDir);
                }
                _ => {}
            },
            Component::CurDir => {}
            _ => {
                clean.push(comp);
            }
        }
    }
    clean
}

/// Resolves a wikilink target relative to the current file and vault root.
/// Guaranteed to:
/// - Prevent escaping the vault root through `../`.
/// - Never double `.md` extensions (e.g. `note.md` -> `note.md`, not `note.md.md`).
/// - Preserve spaces, dots in filenames (`note.v1.0`), nested paths, and Unicode.
/// - Perform shortest-path lookup across the vault for bare note names if not found directly.
pub fn resolve_wikilink_target(
    target: &str,
    current_file: Option<&Path>,
    vault_root: Option<&Path>,
) -> ResolvedLinkTarget {
    let decoded = urlencoding::decode(target).unwrap_or(std::borrow::Cow::Borrowed(target));
    let decoded_str = decoded.trim();

    if decoded_str.is_empty() {
        return ResolvedLinkTarget::CurrentDocument;
    }

    let normalized_target = decoded_str.replace('\\', "/");
    let target_path = PathBuf::from(&normalized_target);

    // Determine candidate directories
    let current_dir = current_file.and_then(|f| f.parent());

    let mut candidate_paths = Vec::new();

    // 1. If target is relative, test relative to current file directory
    if target_path.is_relative()
        && let Some(cd) = current_dir
    {
        candidate_paths.push(cd.join(&target_path));
    }

    // 2. Test relative to vault root
    if target_path.is_relative()
        && let Some(vr) = vault_root
    {
        candidate_paths.push(vr.join(&target_path));
    }

    // 3. Absolute path or fallback to current dir / vault root
    if target_path.is_absolute() {
        candidate_paths.push(target_path.clone());
    }

    let has_md_ext = normalized_target.ends_with(".md") || normalized_target.ends_with(".markdown");
    let media_type = detect_media_type(&target_path);

    // Helper to check existing file
    for candidate in &candidate_paths {
        let cleaned = clean_path(candidate);
        let safe_cleaned = enforce_vault_boundary(&cleaned, vault_root);

        // Check exact match
        if safe_cleaned.exists() {
            if safe_cleaned.is_dir() {
                return ResolvedLinkTarget::Directory {
                    path: safe_cleaned,
                    exists: true,
                };
            }
            if let Some(mt) = detect_media_type(&safe_cleaned) {
                return ResolvedLinkTarget::MediaOrFile {
                    path: safe_cleaned,
                    exists: true,
                    media_type: mt,
                };
            }
            if safe_cleaned.extension().is_some() && !has_md_ext {
                return ResolvedLinkTarget::MediaOrFile {
                    path: safe_cleaned,
                    exists: true,
                    media_type: MediaType::Other,
                };
            }
            return ResolvedLinkTarget::MarkdownFile {
                path: safe_cleaned,
                exists: true,
            };
        }

        // If no extension or not .md, check candidate with .md appended (only if not already .md)
        if !has_md_ext && media_type.is_none() {
            let mut with_md = safe_cleaned.clone();
            let file_name = with_md.file_name().and_then(|s| s.to_str()).unwrap_or("");
            with_md.set_file_name(format!("{}.md", file_name));
            if with_md.exists() && with_md.is_file() {
                return ResolvedLinkTarget::MarkdownFile {
                    path: with_md,
                    exists: true,
                };
            }
        }
    }

    // 4. Obsidian shortest-path vault-wide search:
    // If target has no path separator, scan vault for a matching file name
    if !normalized_target.contains('/')
        && let Some(vr) = vault_root
        && vr.exists()
    {
        let target_name = normalized_target.as_str();
        let target_md_name = if has_md_ext {
            target_name.to_string()
        } else {
            format!("{}.md", target_name)
        };

        if let Some(found) = find_file_in_vault(vr, target_name, &target_md_name) {
            let safe_found = enforce_vault_boundary(&found, vault_root);
            if let Some(mt) = detect_media_type(&safe_found) {
                return ResolvedLinkTarget::MediaOrFile {
                    path: safe_found,
                    exists: true,
                    media_type: mt,
                };
            }
            return ResolvedLinkTarget::MarkdownFile {
                path: safe_found,
                exists: true,
            };
        }
    }

    // 5. Unresolved target
    // Construct default target path relative to vault root or current dir
    let default_base = vault_root.or(current_dir).unwrap_or_else(|| Path::new("."));
    let mut default_path = default_base.join(&target_path);

    if !has_md_ext && media_type.is_none() {
        let file_name = default_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        default_path.set_file_name(format!("{}.md", file_name));
    }

    let cleaned = clean_path(&default_path);
    let safe_path = enforce_vault_boundary(&cleaned, vault_root);

    if let Some(mt) = media_type {
        ResolvedLinkTarget::MediaOrFile {
            path: safe_path,
            exists: false,
            media_type: mt,
        }
    } else {
        ResolvedLinkTarget::MarkdownFile {
            path: safe_path,
            exists: false,
        }
    }
}

/// Recursively searches for a file with matching name inside the vault.
fn find_file_in_vault(dir: &Path, exact_name: &str, md_name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path.file_name()?.to_string_lossy();
        if file_name.starts_with('.') {
            continue;
        }

        if path.is_file() {
            if file_name.eq_ignore_ascii_case(md_name) || file_name.eq_ignore_ascii_case(exact_name)
            {
                return Some(path);
            }
        } else if path.is_dir() {
            subdirs.push(path);
        }
    }

    for subdir in subdirs {
        if let Some(found) = find_file_in_vault(&subdir, exact_name, md_name) {
            return Some(found);
        }
    }

    None
}

/// Clamps paths so they cannot escape the vault root via `../`.
fn enforce_vault_boundary(path: &Path, vault_root: Option<&Path>) -> PathBuf {
    if let Some(vr) = vault_root {
        let clean_vr = clean_path(vr);
        let clean_p = clean_path(path);

        // If the path escapes the vault root, clamp it to vault root
        if !clean_p.starts_with(&clean_vr) {
            let file_name = clean_p.file_name();
            if let Some(name) = file_name {
                return clean_vr.join(name);
            } else {
                return clean_vr;
            }
        }
        clean_p
    } else {
        clean_path(path)
    }
}

/// Normalizes heading text into a GitHub/Obsidian-compatible slug anchor.
pub fn slugify_heading(text: &str) -> String {
    // Strip markdown formatting symbols
    let mut clean_text = String::with_capacity(text.len());
    let mut in_code = false;
    let chars: Vec<char> = text.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let c = chars[idx];
        if c == '`' {
            in_code = !in_code;
            idx += 1;
            continue;
        }
        if !in_code && (c == '*' || c == '_' || c == '~') {
            idx += 1;
            continue;
        }
        clean_text.push(c);
        idx += 1;
    }

    let mut slug = String::new();
    let mut last_was_hyphen = false;

    for c in clean_text.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                slug.push(lc);
            }
            last_was_hyphen = false;
        } else if (c.is_whitespace() || c == '-' || c == '_') && !last_was_hyphen {
            slug.push('-');
            last_was_hyphen = true;
        }
    }

    let mut trimmed = slug.as_str();
    while trimmed.starts_with('-') || trimmed.starts_with('_') {
        trimmed = &trimmed[1..];
    }
    while trimmed.ends_with('-') || trimmed.ends_with('_') {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    trimmed.to_string()
}

/// Finds the line number (0-indexed) of a heading in markdown content.
/// Supports case differences, duplicate headings (`#heading-1`, `#heading-2`),
/// and Unicode characters.
pub fn find_heading_line(text: &str, anchor: &str) -> Option<usize> {
    let target_slug = slugify_heading(anchor);
    let mut heading_counts: HashMap<String, usize> = HashMap::new();

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count <= 6 {
                let rest = &trimmed[hash_count..];
                if rest.is_empty() || rest.chars().next().unwrap().is_whitespace() {
                    let heading_text = rest.trim();
                    let base_slug = slugify_heading(heading_text);

                    let count = heading_counts.entry(base_slug.clone()).or_insert(0);
                    let unique_id = if *count == 0 {
                        base_slug.clone()
                    } else {
                        format!("{}-{}", base_slug, count)
                    };
                    *count += 1;

                    if unique_id == target_slug
                        || base_slug == target_slug
                        || heading_text.eq_ignore_ascii_case(anchor.trim())
                    {
                        return Some(line_idx);
                    }
                }
            }
        }
    }

    None
}

/// Finds the line number (0-indexed) of a block reference in markdown content.
/// Recognizes `^block-id` markers placed at the end of lines or paragraphs.
pub fn find_block_line(text: &str, block_id: &str) -> Option<usize> {
    let clean_id = block_id.trim_start_matches('^').trim();
    if clean_id.is_empty() {
        return None;
    }

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_end();
        if let Some(pos) = trimmed.rfind('^') {
            let id = &trimmed[pos + 1..];
            if id == clean_id && (pos == 0 || trimmed.as_bytes()[pos - 1].is_ascii_whitespace()) {
                return Some(line_idx);
            }
        }
    }

    None
}

/// Extracts a heading section (from the heading line up to the next heading of equal or higher level).
pub fn extract_heading_section(text: &str, anchor: &str) -> Option<String> {
    let target_line = find_heading_line(text, anchor)?;
    let lines: Vec<&str> = text.lines().collect();

    let target_level = {
        let trimmed = lines[target_line].trim();
        trimmed.chars().take_while(|&c| c == '#').count()
    };

    let mut section_lines = Vec::new();
    section_lines.push(lines[target_line]);

    for &line in &lines[target_line + 1..] {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let rest = &trimmed[level..];
            if (rest.is_empty() || rest.chars().next().unwrap().is_whitespace())
                && level <= target_level
            {
                break;
            }
        }
        section_lines.push(line);
    }

    Some(section_lines.join("\n"))
}

/// Extracts the block of text containing `^block-id` (stripping the block reference marker).
pub fn extract_block(text: &str, block_id: &str) -> Option<String> {
    let block_line = find_block_line(text, block_id)?;
    let lines: Vec<&str> = text.lines().collect();

    // Collect contiguous paragraph/block lines around target line
    let mut start = block_line;
    while start > 0 && !lines[start - 1].trim().is_empty() {
        start -= 1;
    }

    let mut end = block_line;
    while end + 1 < lines.len() && !lines[end + 1].trim().is_empty() {
        end += 1;
    }

    let clean_id = block_id.trim_start_matches('^').trim();
    let marker = format!("^{}", clean_id);

    let mut block_text_lines = Vec::new();
    for &line in &lines[start..=end] {
        let trimmed = line.trim_end();
        if let Some(pos) = trimmed.rfind(&marker)
            && (pos == 0 || trimmed.as_bytes()[pos - 1].is_ascii_whitespace())
        {
            let before = &trimmed[..pos].trim_end();
            block_text_lines.push(before.to_string());
        } else {
            block_text_lines.push(line.to_string());
        }
    }

    Some(block_text_lines.join("\n"))
}

/// Preprocessed Markdown output with a line-mapping table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessedMarkdown {
    pub text: String,
    /// Maps line index in `text` to line index in original source content.
    /// `None` indicates lines expanded from an embed.
    pub line_map: Vec<Option<usize>>,
}

/// Preprocesses Markdown containing Obsidian wikilinks and embeds.
///
/// Features:
/// - Transforms `[[note|alias]]` to `[alias](resolved_path)`.
/// - Embeds images `![[image.png|300]]` using the standard image pipeline.
/// - Embeds notes, sections, and blocks inline with cycle detection and recursion depth limits.
/// - Generates fallback cards for PDF, audio, video, unsupported files, and missing notes.
/// - Produces a line-mapping table so checkbox toggles in preview accurately write back to the original source.
pub fn preprocess_markdown_with_embeds(
    content: &str,
    current_file: Option<&Path>,
    vault_root: Option<&Path>,
    call_stack: &mut Vec<PathBuf>,
    depth: usize,
) -> PreprocessedMarkdown {
    const MAX_EMBED_DEPTH: usize = 8;

    let mut output_lines = Vec::new();
    let mut line_map = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let is_empty_content = content.is_empty();

    if is_empty_content {
        return PreprocessedMarkdown {
            text: String::new(),
            line_map: Vec::new(),
        };
    }

    let mut in_fence = false;
    let mut fence_char = b'`';
    let mut fence_len = 0;

    for (orig_line_idx, line) in lines.iter().enumerate() {
        let trimmed_start = line.trim_start();
        let bytes = trimmed_start.as_bytes();

        if !in_fence {
            if bytes.len() >= 3 && (bytes[0] == b'`' || bytes[0] == b'~') {
                let fc = bytes[0];
                let count = bytes.iter().take_while(|&&b| b == fc).count();
                if count >= 3 {
                    in_fence = true;
                    fence_char = fc;
                    fence_len = count;
                    output_lines.push(line.to_string());
                    line_map.push(Some(orig_line_idx));
                    continue;
                }
            }
        } else {
            if bytes.len() >= fence_len && bytes[0] == fence_char {
                let count = bytes.iter().take_while(|&&b| b == fence_char).count();
                if count >= fence_len {
                    in_fence = false;
                }
            }
            output_lines.push(line.to_string());
            line_map.push(Some(orig_line_idx));
            continue;
        }

        if !line.contains("[[") {
            output_lines.push(line.to_string());
            line_map.push(Some(orig_line_idx));
            continue;
        }

        // Parse links on this specific line
        let line_links = parse_wiki_links(line);
        if line_links.is_empty() {
            output_lines.push(line.to_string());
            line_map.push(Some(orig_line_idx));
            continue;
        }

        // Check if the entire line is a single note/section/block embed
        let is_standalone_embed = line_links.len() == 1
            && line_links[0].is_embed
            && line.trim() == line_links[0].raw.trim();

        if is_standalone_embed {
            let link = &line_links[0];
            let target = resolve_wikilink_target(&link.target, current_file, vault_root);

            match target {
                ResolvedLinkTarget::MarkdownFile { path, exists } => {
                    if !exists {
                        let note_label = if !link.target.is_empty() {
                            &link.target
                        } else {
                            "note"
                        };
                        output_lines.push(format!("> ⚠️ Note not found: **{}**", note_label));
                        line_map.push(Some(orig_line_idx));
                        continue;
                    }

                    // Cycle detection and recursion depth limit
                    if call_stack.contains(&path) {
                        output_lines
                            .push(format!("> ⚠️ Circular embed detected: **{}**", link.target));
                        line_map.push(Some(orig_line_idx));
                        continue;
                    }

                    if depth >= MAX_EMBED_DEPTH {
                        output_lines.push(format!(
                            "> ⚠️ Maximum embed depth exceeded: **{}**",
                            link.target
                        ));
                        line_map.push(Some(orig_line_idx));
                        continue;
                    }

                    // Read target note content
                    if let Ok(file_content) = std::fs::read_to_string(&path) {
                        let extracted_content = if let Some(ref h) = link.heading {
                            extract_heading_section(&file_content, h)
                        } else if let Some(ref b) = link.block_id {
                            extract_block(&file_content, b)
                        } else {
                            Some(file_content)
                        };

                        if let Some(embed_raw) = extracted_content {
                            call_stack.push(path.clone());
                            let sub_processed = preprocess_markdown_with_embeds(
                                &embed_raw,
                                Some(&path),
                                vault_root,
                                call_stack,
                                depth + 1,
                            );
                            call_stack.pop();

                            // Render embed frame with header
                            let title = link
                                .alias
                                .as_deref()
                                .unwrap_or(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""));
                            output_lines.push(format!("> 📄 **{}**", title));
                            line_map.push(Some(orig_line_idx));

                            for sub_line in sub_processed.text.lines() {
                                output_lines.push(format!("> {}", sub_line));
                                line_map.push(None); // expanded lines are unmapped
                            }
                        } else if link.heading.is_some() {
                            output_lines.push(format!(
                                "> ⚠️ Heading not found: **#{}** in {}",
                                link.heading.as_deref().unwrap_or(""),
                                link.target
                            ));
                            line_map.push(Some(orig_line_idx));
                        } else if link.block_id.is_some() {
                            output_lines.push(format!(
                                "> ⚠️ Block not found: **^{}** in {}",
                                link.block_id.as_deref().unwrap_or(""),
                                link.target
                            ));
                            line_map.push(Some(orig_line_idx));
                        }
                    } else {
                        output_lines.push(format!("> ⚠️ Failed to read note: **{}**", link.target));
                        line_map.push(Some(orig_line_idx));
                    }
                    continue;
                }
                ResolvedLinkTarget::MediaOrFile {
                    path,
                    exists,
                    media_type,
                } => {
                    let file_name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&link.target);
                    if !exists {
                        output_lines.push(format!("> ⚠️ File not found: **{}**", file_name));
                        line_map.push(Some(orig_line_idx));
                        continue;
                    }

                    match media_type {
                        MediaType::Image => {
                            let alias_str = link.alias.as_deref().unwrap_or(file_name);
                            let uri = format!("file://{}", path.to_string_lossy());
                            output_lines.push(format!("![{}]({})", alias_str, uri));
                            line_map.push(Some(orig_line_idx));
                        }
                        MediaType::Pdf => {
                            let uri = format!("file://{}", path.to_string_lossy());
                            output_lines.push(format!(
                                "> 📕 **[{}]({})** *(PDF Document — click to open)*",
                                file_name, uri
                            ));
                            line_map.push(Some(orig_line_idx));
                        }
                        MediaType::Audio => {
                            let uri = format!("file://{}", path.to_string_lossy());
                            output_lines.push(format!(
                                "> 🎵 **[{}]({})** *(Audio — click to open)*",
                                file_name, uri
                            ));
                            line_map.push(Some(orig_line_idx));
                        }
                        MediaType::Video => {
                            let uri = format!("file://{}", path.to_string_lossy());
                            output_lines.push(format!(
                                "> 🎬 **[{}]({})** *(Video — click to open)*",
                                file_name, uri
                            ));
                            line_map.push(Some(orig_line_idx));
                        }
                        MediaType::Other => {
                            let uri = format!("file://{}", path.to_string_lossy());
                            output_lines.push(format!(
                                "> 📎 **[{}]({})** *(File attachment — click to open)*",
                                file_name, uri
                            ));
                            line_map.push(Some(orig_line_idx));
                        }
                    }
                    continue;
                }
                ResolvedLinkTarget::CurrentDocument => {
                    // Embed within current document (heading or block)
                    if let Some(ref h) = link.heading {
                        if let Some(extracted) = extract_heading_section(content, h) {
                            output_lines.push(format!("> 🔖 **#{}**", h));
                            line_map.push(Some(orig_line_idx));
                            for sub_line in extracted.lines() {
                                output_lines.push(format!("> {}", sub_line));
                                line_map.push(None);
                            }
                        } else {
                            output_lines.push(format!("> ⚠️ Heading not found: **#{}**", h));
                            line_map.push(Some(orig_line_idx));
                        }
                    } else if let Some(ref b) = link.block_id {
                        if let Some(extracted) = extract_block(content, b) {
                            output_lines.push(format!("> 🧱 **^{}**", b));
                            line_map.push(Some(orig_line_idx));
                            for sub_line in extracted.lines() {
                                output_lines.push(format!("> {}", sub_line));
                                line_map.push(None);
                            }
                        } else {
                            output_lines.push(format!("> ⚠️ Block not found: **^{}**", b));
                            line_map.push(Some(orig_line_idx));
                        }
                    }
                    continue;
                }
                ResolvedLinkTarget::Directory { path, .. } => {
                    output_lines.push(format!("> 📁 **{}**", path.to_string_lossy()));
                    line_map.push(Some(orig_line_idx));
                    continue;
                }
            }
        }

        // Inline wikilinks replacement (one or more links on the line)
        let mut transformed = String::new();
        let mut last_end = 0;

        for link in line_links {
            if link.span.start > last_end {
                transformed.push_str(&line[last_end..link.span.start]);
            }

            let resolved = resolve_wikilink_target(&link.target, current_file, vault_root);

            if link.is_embed {
                // Inline embed (e.g. image embed within a sentence)
                match resolved {
                    ResolvedLinkTarget::MediaOrFile {
                        path,
                        exists,
                        media_type,
                    } => {
                        let file_name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&link.target);
                        if exists {
                            let uri = format!("file://{}", path.to_string_lossy());
                            if media_type == MediaType::Image {
                                let alt = link.alias.as_deref().unwrap_or(file_name);
                                transformed.push_str(&format!("![{}]({})", alt, uri));
                            } else {
                                transformed.push_str(&format!("[{}]({})", file_name, uri));
                            }
                        } else {
                            transformed.push_str(&format!("⚠️[Missing: {}]", file_name));
                        }
                    }
                    _ => {
                        transformed.push_str(&format!("[Embed: {}]", link.target));
                    }
                }
            } else {
                // Standard wikilink [[target#heading|alias]]
                let display_text = if let Some(ref a) = link.alias {
                    a.clone()
                } else if !link.target.is_empty() {
                    if let Some(ref h) = link.heading {
                        format!("{} > {}", link.target, h)
                    } else if let Some(ref b) = link.block_id {
                        format!("{} > ^{}", link.target, b)
                    } else {
                        link.target.clone()
                    }
                } else if let Some(ref h) = link.heading {
                    format!("#{}", h)
                } else if let Some(ref b) = link.block_id {
                    format!("^{}", b)
                } else {
                    link.raw.clone()
                };

                let mut dest_url = match &resolved {
                    ResolvedLinkTarget::MarkdownFile { path, .. }
                    | ResolvedLinkTarget::MediaOrFile { path, .. }
                    | ResolvedLinkTarget::Directory { path, .. } => {
                        path.to_string_lossy().into_owned()
                    }
                    ResolvedLinkTarget::CurrentDocument => String::new(),
                };

                if let Some(ref h) = link.heading {
                    dest_url.push('#');
                    dest_url.push_str(&slugify_heading(h));
                } else if let Some(ref b) = link.block_id {
                    dest_url.push('#');
                    dest_url.push('^');
                    dest_url.push_str(b.trim_start_matches('^'));
                }

                transformed.push_str(&format!("[{}]({})", display_text, dest_url));
            }

            last_end = link.span.end;
        }

        if last_end < line.len() {
            transformed.push_str(&line[last_end..]);
        }

        output_lines.push(transformed);
        line_map.push(Some(orig_line_idx));
    }

    PreprocessedMarkdown {
        text: output_lines.join("\n"),
        line_map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_wikilink_parse_basic_and_aliases() {
        let content = "Check [[note]] and [[note|alias]] and [[folder/doc|My Doc]].";
        let links = parse_wiki_links(content);
        assert_eq!(links.len(), 3);

        assert_eq!(links[0].target, "note");
        assert_eq!(links[0].alias, None);
        assert!(!links[0].is_embed);

        assert_eq!(links[1].target, "note");
        assert_eq!(links[1].alias, Some("alias".to_string()));
        assert!(!links[1].is_embed);

        assert_eq!(links[2].target, "folder/doc");
        assert_eq!(links[2].alias, Some("My Doc".to_string()));
    }

    #[test]
    fn test_wikilink_no_double_md_extension() {
        let root = std::env::temp_dir().join("dr_md_test_no_double_md");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let note_path = root.join("test_note.md");
        fs::write(&note_path, "# Test").unwrap();

        // Target with .md already
        let resolved = resolve_wikilink_target("test_note.md", None, Some(&root));
        match resolved {
            ResolvedLinkTarget::MarkdownFile { path, exists } => {
                assert!(exists);
                assert_eq!(path, note_path);
                assert!(!path.to_string_lossy().ends_with(".md.md"));
            }
            _ => panic!("Expected MarkdownFile"),
        }

        // Target without .md
        let resolved2 = resolve_wikilink_target("test_note", None, Some(&root));
        match resolved2 {
            ResolvedLinkTarget::MarkdownFile { path, exists } => {
                assert!(exists);
                assert_eq!(path, note_path);
                assert!(!path.to_string_lossy().ends_with(".md.md"));
            }
            _ => panic!("Expected MarkdownFile"),
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_wikilink_nested_paths_spaces_unicode_dots() {
        let root = std::env::temp_dir().join("dr_md_test_unicode_dots");
        let _ = fs::remove_dir_all(&root);
        let sub = root.join("sub").join("nested");
        fs::create_dir_all(&sub).unwrap();

        let unicode_file = sub.join("Заметка.v1.0.md");
        fs::write(&unicode_file, "# Заголовок").unwrap();

        let file_with_space = root.join("Note With Space.md");
        fs::write(&file_with_space, "# Spaced Note").unwrap();

        // Nested with dots and unicode
        let res1 = resolve_wikilink_target("sub/nested/Заметка.v1.0", None, Some(&root));
        assert!(res1.exists());
        assert_eq!(res1.path().unwrap(), unicode_file);

        // Spaces and URL-encoded
        let res2 = resolve_wikilink_target("Note With Space", None, Some(&root));
        assert!(res2.exists());
        assert_eq!(res2.path().unwrap(), file_with_space);

        let res3 = resolve_wikilink_target("Note%20With%20Space", None, Some(&root));
        assert!(res3.exists());
        assert_eq!(res3.path().unwrap(), file_with_space);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_wikilink_heading_anchors_and_duplicates() {
        let doc = r#"# Overview
Some text

## Section
First section text

## Section
Second duplicate section text

### **Bold** Heading!
More text

# Заголовок раздела
Unicode heading
"#;

        assert_eq!(find_heading_line(doc, "overview"), Some(0));
        assert_eq!(find_heading_line(doc, "Overview"), Some(0));

        // First duplicate heading
        assert_eq!(find_heading_line(doc, "section"), Some(3));
        // Second duplicate heading
        assert_eq!(find_heading_line(doc, "section-1"), Some(6));

        // Formatted heading
        assert_eq!(find_heading_line(doc, "bold-heading"), Some(9));

        // Unicode heading
        assert_eq!(find_heading_line(doc, "заголовок-раздела"), Some(12));
    }

    #[test]
    fn test_wikilink_block_references() {
        let doc = r#"Paragraph one with reference. ^p1

- [ ] Task with block ref ^task-99

Normal line without ref"#;

        assert_eq!(find_block_line(doc, "p1"), Some(0));
        assert_eq!(find_block_line(doc, "^p1"), Some(0));
        assert_eq!(find_block_line(doc, "task-99"), Some(2));
        assert_eq!(find_block_line(doc, "non-existent"), None);

        // Extract block
        let extracted = extract_block(doc, "p1").unwrap();
        assert_eq!(extracted, "Paragraph one with reference.");
    }

    #[test]
    fn test_wikilink_embed_note_heading_block() {
        let root = std::env::temp_dir().join("dr_md_test_embeds");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let note_a = root.join("NoteA.md");
        fs::write(
            &note_a,
            "# Heading A\nLine A1\nLine A2\n\n## Subheading\nSub line ^block-a\n\n# Heading B\nLine B1",
        )
        .unwrap();

        // 1. Full note embed
        let res_full =
            preprocess_markdown_with_embeds("![[NoteA]]", None, Some(&root), &mut Vec::new(), 0);
        assert!(res_full.text.contains("Line A1"));
        assert!(res_full.text.contains("Line B1"));

        // 2. Heading embed
        let res_head = preprocess_markdown_with_embeds(
            "![[NoteA#Subheading]]",
            None,
            Some(&root),
            &mut Vec::new(),
            0,
        );
        assert!(res_head.text.contains("Sub line"));
        assert!(!res_head.text.contains("Line B1"));

        // 3. Block embed
        let res_block = preprocess_markdown_with_embeds(
            "![[NoteA^block-a]]",
            None,
            Some(&root),
            &mut Vec::new(),
            0,
        );
        assert!(res_block.text.contains("Sub line"));
        assert!(!res_block.text.contains("^block-a")); // stripped marker

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_wikilink_image_embeds() {
        let root = std::env::temp_dir().join("dr_md_test_img");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let img_path = root.join("photo.png");
        fs::write(&img_path, b"fake png data").unwrap();

        let res = preprocess_markdown_with_embeds(
            "![[photo.png|300]]",
            None,
            Some(&root),
            &mut Vec::new(),
            0,
        );
        assert!(res.text.contains("![300](file://"));
        assert!(res.text.contains("photo.png"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_wikilink_embed_cycle_detection() {
        let root = std::env::temp_dir().join("dr_md_test_cycle");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let file_a = root.join("CycleA.md");
        let file_b = root.join("CycleB.md");

        fs::write(&file_a, "Note A content\n![[CycleB]]").unwrap();
        fs::write(&file_b, "Note B content\n![[CycleA]]").unwrap();

        let res =
            preprocess_markdown_with_embeds("![[CycleA]]", None, Some(&root), &mut Vec::new(), 0);
        assert!(res.text.contains("Circular embed detected"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_wikilink_vault_traversal_protection() {
        let root = std::env::temp_dir().join("dr_md_test_traversal_vault");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let resolved = resolve_wikilink_target("../../../../etc/passwd", None, Some(&root));
        if let Some(path) = resolved.path() {
            assert!(
                path.starts_with(&root),
                "Path {:?} should not escape root {:?}",
                path,
                root
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_ordinary_markdown_links_unchanged() {
        let input = "Check [Google](https://google.com) and [Local](local.md) link.";
        let res = preprocess_markdown_with_embeds(input, None, None, &mut Vec::new(), 0);
        assert_eq!(res.text, input);
    }

    #[test]
    fn test_code_blocks_and_spans_ignored() {
        let input = "```markdown\n[[raw wikilink inside fence]]\n```\nAnd `[[inline code]]`.";
        let res = preprocess_markdown_with_embeds(input, None, None, &mut Vec::new(), 0);
        assert_eq!(res.text, input);
    }

    #[test]
    fn test_checkbox_write_back_line_map() {
        let root = std::env::temp_dir().join("dr_md_test_line_map");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let embedded_note = root.join("Emb.md");
        fs::write(&embedded_note, "Line 1\nLine 2\nLine 3").unwrap();

        let main_doc = "- [ ] Task 1\n![[Emb]]\n- [ ] Task 2";
        let res = preprocess_markdown_with_embeds(main_doc, None, Some(&root), &mut Vec::new(), 0);

        // First line is Task 1 -> orig_line 0
        assert_eq!(res.line_map[0], Some(0));

        // Embed lines have header mapped to Some(1), and expanded lines mapped to None
        assert_eq!(res.line_map[1], Some(1));
        assert_eq!(res.line_map[2], None);
        assert_eq!(res.line_map[3], None);
        assert_eq!(res.line_map[4], None);

        // Last line is Task 2 -> orig_line 2
        let last_idx = res.line_map.len() - 1;
        assert_eq!(res.line_map[last_idx], Some(2));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_unresolved_links_and_creation_path() {
        let root = std::env::temp_dir().join("dr_md_test_unresolved");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        // 1. Unresolved note link
        let res = resolve_wikilink_target("FutureNote", None, Some(&root));
        match res {
            ResolvedLinkTarget::MarkdownFile { path, exists } => {
                assert!(!exists);
                assert_eq!(path, root.join("FutureNote.md"));
                assert!(!path.to_string_lossy().ends_with(".md.md"));

                // Simulate creation
                if let Some(p) = path.parent() {
                    fs::create_dir_all(p).unwrap();
                }
                fs::write(&path, "# Future Note").unwrap();
            }
            _ => panic!("Expected MarkdownFile"),
        }

        // 2. Now resolves as existing
        let res2 = resolve_wikilink_target("FutureNote", None, Some(&root));
        match res2 {
            ResolvedLinkTarget::MarkdownFile { path, exists } => {
                assert!(exists);
                assert_eq!(path, root.join("FutureNote.md"));
            }
            _ => panic!("Expected MarkdownFile"),
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_pdf_and_unsupported_embed_fallbacks() {
        let root = std::env::temp_dir().join("dr_md_test_fallbacks");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let pdf = root.join("doc.pdf");
        fs::write(&pdf, b"%PDF fake content").unwrap();

        let zip = root.join("data.zip");
        fs::write(&zip, b"PK fake zip content").unwrap();

        let input = "![[doc.pdf]]\n![[data.zip]]\n![[nonexistent.pdf]]";
        let res = preprocess_markdown_with_embeds(input, None, Some(&root), &mut Vec::new(), 0);

        assert!(res.text.contains("doc.pdf"));
        assert!(res.text.contains("PDF Document — click to open"));
        assert!(res.text.contains("data.zip"));
        assert!(res.text.contains("File attachment — click to open"));
        assert!(res.text.contains("File not found: **nonexistent.pdf**"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_lossless_source_with_complex_wikilinks() {
        let original = r#"# Notes on [[Quantum Mechanics|QM]]
See also [[Chapter 1#Introduction]] and [[#Summary]].
- [ ] Read section ^sec-1
- [x] Completed task

Unicode link: [[Заметки/Физика#Введение|Введение в физику]]
"#;
        // The original source string in the editor remains untouched and unmodified
        let links = parse_wiki_links(original);
        assert_eq!(links.len(), 4);
        assert_eq!(links[0].target, "Quantum Mechanics");
        assert_eq!(links[0].alias, Some("QM".to_string()));
        assert_eq!(links[1].target, "Chapter 1");
        assert_eq!(links[1].heading, Some("Introduction".to_string()));
        assert_eq!(links[2].target, "");
        assert_eq!(links[2].heading, Some("Summary".to_string()));
        assert_eq!(links[3].target, "Заметки/Физика");
        assert_eq!(links[3].heading, Some("Введение".to_string()));
        assert_eq!(links[3].alias, Some("Введение в физику".to_string()));
    }
}
