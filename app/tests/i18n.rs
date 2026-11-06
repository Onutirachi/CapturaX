//! Translations (FR-039, research R7): every `@tr` text of the UI has a translation in every
//! catalog, no catalog keeps texts the UI no longer has, and no user-facing property is set to a
//! plain string literal.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Properties that show text to the user: a name ending in one of these (`text`, `tooltip`,
/// `accessible-label`, ...) must not be set to a non-empty literal without `@tr`.
const USER_FACING: [&str; 4] = ["text", "title", "tooltip", "label"];

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn files_with_extension(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            files_with_extension(&path, extension, out);
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
    out.sort();
}

/// A string literal found in a `.slint` file.
struct Literal {
    value: String,
    line: usize,
    /// Starts right after `@tr(`.
    translated: bool,
    /// The property the literal's statement assigns, if any.
    property: Option<String>,
}

/// Finds the string literals of a `.slint` source, skipping comments.
fn literals(source: &str) -> Vec<Literal> {
    let chars: Vec<char> = source.chars().collect();
    let mut found = Vec::new();
    let mut line = 1;
    // Where the current statement starts: after the last `;`, `{` or `}`, or a `,` outside
    // parentheses (struct literal fields).
    let mut statement = 0;
    let mut depth = 0usize;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\n' => line += 1,
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ';' | '{' | '}' => {
                statement = i + 1;
                depth = 0;
            }
            ',' if depth == 0 => statement = i + 1,
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                i += 2;
                continue;
            }
            '"' => {
                let start_line = line;
                let before: String = chars[statement..i].iter().collect();
                let translated = before.trim_end().ends_with("@tr(");
                let mut value = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    match chars[i] {
                        '\\' => {
                            i += 1;
                            match chars.get(i) {
                                Some('n') => value.push('\n'),
                                Some('t') => value.push('\t'),
                                Some('"') => value.push('"'),
                                Some('\\') => value.push('\\'),
                                Some('{') => {
                                    panic!("line {start_line}: `\\{{` (interpolation) is not handled by this test")
                                }
                                Some(&c) => {
                                    value.push('\\');
                                    value.push(c);
                                }
                                None => {}
                            }
                        }
                        '\n' => {
                            line += 1;
                            value.push('\n');
                        }
                        c => value.push(c),
                    }
                    i += 1;
                }
                if translated {
                    let after: String = chars[i + 1..].iter().take_while(|c| **c != ')').collect();
                    let after = after.trim_start();
                    assert!(
                        !after.starts_with("=>") && !after.starts_with('|'),
                        "line {start_line}: @tr with a context or plural is not handled by this test"
                    );
                }
                found.push(Literal {
                    value,
                    line: start_line,
                    translated,
                    property: property(&before),
                });
            }
            _ => {}
        }
        i += 1;
    }
    found
}

/// The property a statement assigns: the first `name:` in it.
fn property(statement: &str) -> Option<String> {
    let colon = statement.find(':')?;
    let name: String = statement[..colon]
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (!name.is_empty()).then_some(name)
}

/// One catalog entry.
struct Entry {
    msgid: String,
    msgstr: String,
    fuzzy: bool,
}

/// Reads the entries of a `.po` file (no contexts or plurals; obsolete entries skipped).
fn catalog(source: &str, path: &Path) -> Vec<Entry> {
    #[derive(Clone, Copy)]
    enum Field {
        None,
        Id,
        Str,
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut field = Field::None;
    // A `#, fuzzy` flag read for the entry that comes next.
    let mut fuzzy_next = false;
    for (n, line) in source.lines().enumerate() {
        let line = line.trim();
        let at = || format!("{}:{}", path.display(), n + 1);
        if line.is_empty() || line.starts_with("#~") {
            continue;
        }
        if let Some(flags) = line.strip_prefix("#,") {
            field = Field::None;
            fuzzy_next = flags.split(',').any(|f| f.trim() == "fuzzy");
        } else if line.starts_with('#') {
            continue;
        } else if let Some(rest) = line.strip_prefix("msgid ") {
            entries.push(Entry {
                msgid: po_string(rest, &at()),
                msgstr: String::new(),
                fuzzy: fuzzy_next,
            });
            fuzzy_next = false;
            field = Field::Id;
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            let entry = entries
                .last_mut()
                .unwrap_or_else(|| panic!("{}: msgstr without msgid", at()));
            entry.msgstr = po_string(rest, &at());
            field = Field::Str;
        } else if line.starts_with('"') {
            let more = po_string(line, &at());
            let entry = entries.last_mut();
            match (field, entry) {
                (Field::Id, Some(entry)) => entry.msgid.push_str(&more),
                (Field::Str, Some(entry)) => entry.msgstr.push_str(&more),
                _ => panic!("{}: text outside an entry", at()),
            }
        } else if line.starts_with("msgctxt") || line.starts_with("msgid_plural") || line.starts_with("msgstr[") {
            panic!("{}: contexts and plurals are not handled by this test", at());
        } else {
            panic!("{}: unexpected line", at());
        }
    }

    entries
}

/// Unescapes one quoted `.po` string.
fn po_string(quoted: &str, at: &str) -> String {
    let inner = quoted
        .trim()
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{at}: expected a quoted string"));
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            other => panic!("{at}: unknown escape \\{}", other.map(String::from).unwrap_or_default()),
        }
    }
    out
}

/// Every `.slint` file of the UI with its literals. `ui/previews` is left out: the previews are
/// for `slint-viewer` only, never built into the app, and show sample texts.
fn ui_literals() -> Vec<(PathBuf, Vec<Literal>)> {
    let mut files = Vec::new();
    files_with_extension(&manifest().join("ui"), "slint", &mut files);
    let previews = manifest().join("ui").join("previews");
    files.retain(|path| !path.starts_with(&previews));
    assert!(!files.is_empty(), "no .slint files found");
    files
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let found = literals(&source);
            (path, found)
        })
        .collect()
}

fn relative(path: &Path) -> String {
    path.strip_prefix(manifest())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

#[test]
fn every_catalog_matches_the_ui() {
    let texts: BTreeSet<String> = ui_literals()
        .into_iter()
        .flat_map(|(_, found)| found.into_iter().filter(|l| l.translated).map(|l| l.value))
        .collect();
    assert!(!texts.is_empty(), "no @tr texts found in the UI");

    let mut catalogs = Vec::new();
    files_with_extension(&manifest().join("lang"), "po", &mut catalogs);
    assert!(!catalogs.is_empty(), "no .po catalogs found");

    let mut failures = Vec::new();
    for path in catalogs {
        let name = relative(&path);
        let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let entries: Vec<Entry> = catalog(&source, &path)
            .into_iter()
            .filter(|e| !e.msgid.is_empty())
            .collect();
        let ids: BTreeSet<&str> = entries.iter().map(|e| e.msgid.as_str()).collect();
        for text in &texts {
            if !ids.contains(text.as_str()) {
                failures.push(format!("{name}: missing {text:?}"));
            }
        }
        for entry in &entries {
            if !texts.contains(&entry.msgid) {
                failures.push(format!("{name}: stale {:?} (no longer in the UI)", entry.msgid));
            } else if entry.msgstr.is_empty() {
                failures.push(format!("{name}: empty translation for {:?}", entry.msgid));
            } else if entry.fuzzy {
                failures.push(format!("{name}: fuzzy translation for {:?}", entry.msgid));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "catalogs out of step with the UI:\n{}",
        failures.join("\n")
    );
}

#[test]
fn user_facing_properties_use_tr() {
    let mut failures = Vec::new();
    for (path, found) in ui_literals() {
        for literal in found {
            let Some(property) = literal.property.as_deref() else {
                continue;
            };
            let user_facing = USER_FACING
                .iter()
                .any(|p| property == *p || property.ends_with(&format!("-{p}")));
            if user_facing && !literal.translated && !literal.value.is_empty() {
                failures.push(format!(
                    "{}:{}: {property} = {:?} without @tr",
                    relative(&path),
                    literal.line,
                    literal.value
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "user-facing literals without @tr:\n{}",
        failures.join("\n")
    );
}

#[test]
fn extraction_handles_escapes_placeholders_and_literals() {
    let source = r#"
        // text: "a comment";
        Text { text: @tr("Say \"hi\" {}", name); }
        Button { tooltip: on ? @tr("On") : "Off"; accessible-label: ""; }
        Select { items: [{ id: 1, text: "Raw" }]; }
        import { X } from "x.slint";
    "#;
    let found = literals(source);
    let values: Vec<(&str, bool, Option<&str>)> = found
        .iter()
        .map(|l| (l.value.as_str(), l.translated, l.property.as_deref()))
        .collect();
    assert_eq!(
        values,
        [
            ("Say \"hi\" {}", true, Some("text")),
            ("On", true, Some("tooltip")),
            ("Off", false, Some("tooltip")),
            ("", false, Some("accessible-label")),
            ("Raw", false, Some("text")),
            ("x.slint", false, None),
        ]
    );
    assert_eq!(found[0].line, 3);

    let po = "msgid \"\"\nmsgstr \"\"\n\"Language: pt_BR\\n\"\n\n#, fuzzy\nmsgid \"A \\\"b\\\"\"\nmsgstr \"\"\n\"C\"\n\nmsgid \"D\"\nmsgstr \"\"\n";
    let entries = catalog(po, Path::new("test.po"));
    let read: Vec<(&str, &str, bool)> = entries
        .iter()
        .map(|e| (e.msgid.as_str(), e.msgstr.as_str(), e.fuzzy))
        .collect();
    assert_eq!(
        read,
        [
            ("", "Language: pt_BR\n", false),
            ("A \"b\"", "C", true),
            ("D", "", false)
        ]
    );
}
