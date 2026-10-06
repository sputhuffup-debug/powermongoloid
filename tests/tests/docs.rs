//! M3b: the developer handbook stays in step with the code.
//! - Every field in `assets/data/*.ron` is documented in `docs/dev/DATA.md`.
//! - Every repository path named in the handbook exists.
//! - Every relative Markdown link in the handbook, README and CONTRIBUTING
//!   resolves to a file.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests/ sits in the repo root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Removes `//` comments and the contents of string literals.
fn strip_comments_and_strings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_string = false;
                out.push('"');
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push('"');
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Field names in a RON file: identifiers directly followed by `:`.
fn ron_fields(text: &str) -> BTreeSet<String> {
    let text = strip_comments_and_strings(text);
    let mut fields = BTreeSet::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_lowercase() || c == '_' {
            let start = i;
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
            let ident: String = bytes[start..i].iter().collect();
            let mut j = i;
            while j < bytes.len() && bytes[j].is_whitespace() {
                j += 1;
            }
            let preceded_by_ident = start > 0 && is_ident(bytes[start - 1]);
            if !preceded_by_ident && j < bytes.len() && bytes[j] == ':' {
                fields.insert(ident);
            }
        } else {
            i += 1;
        }
    }
    fields
}

/// Words inside `backtick` spans of one `## <file>` section of DATA.md.
fn documented_words(data_md: &str, file: &str) -> BTreeSet<String> {
    let heading = format!("## {file}");
    let start = data_md
        .find(&heading)
        .unwrap_or_else(|| panic!("DATA.md has no section `{heading}`"));
    let rest = &data_md[start + heading.len()..];
    let section = &rest[..rest.find("\n## ").unwrap_or(rest.len())];
    section
        .split('`')
        .skip(1)
        .step_by(2)
        .flat_map(|span| span.split(|c: char| !is_ident(c)))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn every_data_field_is_documented() {
    let data_md = read("docs/dev/DATA.md");
    let mut files: Vec<PathBuf> = fs::read_dir(root().join("assets/data"))
        .expect("assets/data")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let mut missing = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let fields = ron_fields(&fs::read_to_string(&path).expect("readable"));
        let words = documented_words(&data_md, &name);
        for f in fields.difference(&words) {
            missing.push(format!("{name}: {f}"));
        }
    }
    assert!(
        missing.is_empty(),
        "fields missing from docs/dev/DATA.md (add a row):\n  {}",
        missing.join("\n  ")
    );
}

/// Backticked repository paths such as `crates/sim_core/src/lib.rs`.
fn named_paths(md: &str) -> Vec<String> {
    const ROOTS: [&str; 6] = [
        "crates/", "tests/", "assets/", "scripts/", "docs/", ".github/",
    ];
    md.split('`')
        .skip(1)
        .step_by(2)
        .filter(|s| ROOTS.iter().any(|r| s.starts_with(r)) && !s.contains(' ') && !s.contains('*'))
        .map(|s| s.trim_end_matches('/').to_string())
        .collect()
}

#[test]
fn howto_paths_exist() {
    let mut missing = Vec::new();
    for doc in [
        "docs/dev/HOWTO.md",
        "docs/dev/ARCHITECTURE.md",
        "docs/dev/TESTING.md",
        "docs/dev/GETTING_STARTED.md",
        "docs/dev/DATA.md",
        "CONTRIBUTING.md",
    ] {
        for p in named_paths(&read(doc)) {
            if !root().join(&p).exists() {
                missing.push(format!("{doc}: {p}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "paths named in the handbook that don't exist:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn markdown_links_resolve() {
    let mut docs: Vec<PathBuf> = fs::read_dir(root().join("docs/dev"))
        .expect("docs/dev")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    docs.push(root().join("README.md"));
    docs.push(root().join("CONTRIBUTING.md"));
    let mut broken = Vec::new();
    for doc in docs {
        let text = fs::read_to_string(&doc).expect("readable");
        for part in text.split("](").skip(1) {
            let Some(end) = part.find(')') else { continue };
            let target = &part[..end];
            if target.starts_with("http") || target.starts_with('#') || target.is_empty() {
                continue;
            }
            let file = target.split('#').next().unwrap_or(target);
            let resolved = doc.parent().unwrap().join(file);
            if !resolved.exists() {
                broken.push(format!("{}: {target}", doc.display()));
            }
        }
    }
    assert!(
        broken.is_empty(),
        "broken links:\n  {}",
        broken.join("\n  ")
    );
}

#[test]
fn field_extraction_finds_nested_fields_and_skips_comments() {
    let ron = r#"// a: comment
        ( top: 1, nested: (inner_one: 2, "str: x"), list: [(name: "a")] )"#;
    let fields = ron_fields(ron);
    let expected: BTreeSet<String> = ["top", "nested", "inner_one", "list", "name"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(fields, expected);
}
