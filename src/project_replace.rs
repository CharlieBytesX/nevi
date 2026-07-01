use std::fs;
use std::path::{Path, PathBuf};

pub const PROJECT_REPLACE_MAX_MATCHES: usize = 5_000;

#[derive(Clone, Debug)]
pub struct ProjectReplacePreview {
    pub root: PathBuf,
    pub pattern: String,
    pub replacement: String,
    pub global: bool,
    pub files_scanned: usize,
    pub skipped_files: usize,
    pub truncated: bool,
    pub files: Vec<ProjectReplaceFile>,
}

#[derive(Clone, Debug)]
pub struct ProjectReplaceFile {
    pub path: PathBuf,
    pub original_content: String,
    pub new_content: String,
    pub line_previews: Vec<ProjectReplaceLinePreview>,
    pub replacements: usize,
}

#[derive(Clone, Debug)]
pub struct ProjectReplaceLinePreview {
    pub line_number: usize,
    pub before: String,
    pub after: String,
    pub replacements: usize,
}

impl ProjectReplacePreview {
    pub fn changed_file_count(&self) -> usize {
        self.files.len()
    }

    pub fn total_replacements(&self) -> usize {
        self.files.iter().map(|file| file.replacements).sum()
    }

    pub fn render_markdown(&self) -> String {
        let mut output = String::new();
        output.push_str("# Project Replace Preview\n\n");
        output.push_str(&format!("Pattern: `{}`\n", self.pattern));
        output.push_str(&format!("Replacement: `{}`\n", self.replacement));
        output.push_str(&format!("Scope: `{}`\n", self.root.display()));
        output.push_str(&format!(
            "Mode: {}\n",
            if self.global {
                "all matches per line"
            } else {
                "first match per line"
            }
        ));
        output.push_str(&format!("Files scanned: {}\n", self.files_scanned));
        output.push_str(&format!("Files skipped: {}\n", self.skipped_files));
        output.push_str(&format!("Files changed: {}\n", self.changed_file_count()));
        output.push_str(&format!("Replacements: {}\n\n", self.total_replacements()));

        if self.truncated {
            output.push_str(
                "Preview truncated before all matches were collected. Apply is disabled.\n\n",
            );
        } else if self.total_replacements() > 0 {
            output.push_str("Apply with `:ProjectReplaceApply`.\n\n");
        } else {
            output.push_str("No matches found.\n\n");
        }

        for file in &self.files {
            output.push_str(&format!("## {}\n", display_path(&self.root, &file.path)));
            output.push_str(&format!("{} replacement(s)\n\n", file.replacements));

            for line in &file.line_previews {
                output.push_str(&format!(
                    "- line {}: {} replacement(s)\n",
                    line.line_number, line.replacements
                ));
                output.push_str(&format!("  before: `{}`\n", line.before));
                output.push_str(&format!("  after:  `{}`\n", line.after));
            }
            output.push('\n');
        }

        output
    }
}

pub fn build_project_replace_preview(
    root: &Path,
    files: impl IntoIterator<Item = PathBuf>,
    pattern: &str,
    replacement: &str,
    global: bool,
    max_matches: usize,
) -> Result<ProjectReplacePreview, String> {
    if pattern.is_empty() {
        return Err("ProjectReplace requires a non-empty pattern".to_string());
    }

    let mut preview = ProjectReplacePreview {
        root: root.to_path_buf(),
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
        global,
        files_scanned: 0,
        skipped_files: 0,
        truncated: false,
        files: Vec::new(),
    };

    for path in files {
        if preview.total_replacements() >= max_matches {
            preview.truncated = true;
            break;
        }

        let Ok(original_content) = fs::read_to_string(&path) else {
            preview.skipped_files += 1;
            continue;
        };
        preview.files_scanned += 1;

        let remaining = max_matches.saturating_sub(preview.total_replacements());
        let replaced = replace_content(&original_content, pattern, replacement, global, remaining);
        if replaced.truncated {
            preview.truncated = true;
        }

        if replaced.replacements > 0 {
            let new_content = if replaced.truncated {
                original_content.clone()
            } else {
                replaced.content
            };
            preview.files.push(ProjectReplaceFile {
                path,
                original_content,
                new_content,
                line_previews: replaced.line_previews,
                replacements: replaced.replacements,
            });
        }

        if preview.truncated {
            break;
        }
    }

    Ok(preview)
}

struct ContentReplacement {
    content: String,
    line_previews: Vec<ProjectReplaceLinePreview>,
    replacements: usize,
    truncated: bool,
}

fn replace_content(
    content: &str,
    pattern: &str,
    replacement: &str,
    global: bool,
    max_matches: usize,
) -> ContentReplacement {
    let mut output = String::with_capacity(content.len());
    let mut line_previews = Vec::new();
    let mut replacements = 0;
    let mut truncated = false;

    for (line_index, line) in content.split_inclusive('\n').enumerate() {
        let (line_body, line_ending) = split_line_ending(line);
        let (new_body, count) = replace_line_literal(line_body, pattern, replacement, global);

        if count > 0 {
            if replacements + count > max_matches {
                truncated = true;
                output.push_str(line);
                break;
            }

            line_previews.push(ProjectReplaceLinePreview {
                line_number: line_index + 1,
                before: line_body.to_string(),
                after: new_body.clone(),
                replacements: count,
            });
            replacements += count;
            output.push_str(&new_body);
            output.push_str(line_ending);
        } else {
            output.push_str(line);
        }
    }

    ContentReplacement {
        content: output,
        line_previews,
        replacements,
        truncated,
    }
}

fn replace_line_literal(
    line: &str,
    pattern: &str,
    replacement: &str,
    global: bool,
) -> (String, usize) {
    if global {
        let count = line.matches(pattern).count();
        if count == 0 {
            (line.to_string(), 0)
        } else {
            (line.replace(pattern, replacement), count)
        }
    } else if let Some(_) = line.find(pattern) {
        (line.replacen(pattern, replacement, 1), 1)
    } else {
        (line.to_string(), 0)
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(stripped) = line.strip_suffix("\r\n") {
        (stripped, "\r\n")
    } else if let Some(stripped) = line.strip_suffix('\n') {
        (stripped, "\n")
    } else {
        (line, "")
    }
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos))
    }

    #[test]
    fn project_replace_preview_does_not_mutate_files() {
        let root = unique_temp_dir("nevi_project_replace_preview");
        fs::create_dir_all(root.join("src")).expect("create temp tree");
        let first = root.join("src/first.txt");
        let second = root.join("second.txt");
        fs::write(&first, "old one\nold old\n").expect("write first");
        fs::write(&second, "keep old\n").expect("write second");

        let preview = build_project_replace_preview(
            &root,
            vec![first.clone(), second.clone()],
            "old",
            "new",
            true,
            100,
        )
        .expect("build preview");

        assert_eq!(preview.changed_file_count(), 2);
        assert_eq!(preview.total_replacements(), 4);
        assert!(!preview.truncated);

        let rendered = preview.render_markdown();
        assert!(rendered.contains("# Project Replace Preview"));
        assert!(rendered.contains("Pattern: `old`"));
        assert!(rendered.contains("Replacement: `new`"));
        assert!(rendered.contains("src/first.txt"));
        assert!(rendered.contains("second.txt"));
        assert!(rendered.contains("Apply with `:ProjectReplaceApply`"));

        assert_eq!(
            fs::read_to_string(&first).expect("read first"),
            "old one\nold old\n"
        );
        assert_eq!(
            fs::read_to_string(&second).expect("read second"),
            "keep old\n"
        );
    }

    #[test]
    fn project_replace_preview_marks_truncated_when_match_cap_is_hit() {
        let root = unique_temp_dir("nevi_project_replace_truncated");
        fs::create_dir_all(&root).expect("create temp dir");
        let path = root.join("large.txt");
        fs::write(&path, "old old old\n").expect("write file");

        let preview =
            build_project_replace_preview(&root, vec![path.clone()], "old", "new", true, 2)
                .expect("build preview");

        assert!(preview.truncated);
        assert!(preview.render_markdown().contains("Apply is disabled"));
        assert_eq!(
            fs::read_to_string(&path).expect("read file"),
            "old old old\n"
        );
    }
}
