//! The Deterministic Code Graph (DCG) — layer 1 of the hybrid two-layer code
//! context. Tree-sitter parses code into symbols (functions, classes, etc.)
//! with exact spans; the DCG stores per-file symbol sets + cross-file edges
//! (calls, imports, inheritance) in a later task.
//!
//! DCG handles STRUCTURAL facts (calls, imports, types, refs) — fast, exact,
//! free (no LLM). Graphiti (rai-python) handles SEMANTIC/TEMPORAL facts.
//!
//! T28: the Dcg struct + FileNode + Symbol + SymbolKind.
//! T29: tree-sitter parse a Rust source string into Vec<Symbol>.
//! T30: Dcg::update_file incremental (re-parse one file, replace its node).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tree_sitter::{Parser, Point};

/// A symbol kind (the structural categories tree-sitter extracts).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// A function (`fn foo() {}`).
    Function,
    /// A struct.
    Struct,
    /// An enum.
    Enum,
    /// An impl block (a set of methods for a type).
    Impl,
    /// A method (a function inside an impl block).
    Method,
    /// A trait.
    Trait,
    /// A module (`mod foo {}` or `mod foo;`).
    Module,
    /// A constant or static.
    Constant,
    /// A type alias.
    TypeAlias,
}

/// A symbol extracted from a file (name + span + kind).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Symbol {
    /// The symbol name (e.g. "foo", "Bar", "Baz").
    pub name: String,
    /// The kind.
    pub kind: SymbolKind,
    /// The start position (0-indexed row, 0-indexed column).
    pub start: Position,
    /// The end position.
    pub end: Position,
}

/// A position in a source file (row, column — both 0-indexed).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    /// The 0-indexed row (line).
    pub row: usize,
    /// The 0-indexed column (byte offset within the line).
    pub column: usize,
}

impl From<Point> for Position {
    fn from(p: Point) -> Self {
        Self {
            row: p.row,
            column: p.column,
        }
    }
}

/// A file's node in the DCG (its symbols + detected language).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileNode {
    /// The file path.
    pub path: PathBuf,
    /// The detected language (e.g. "rust", "python").
    pub language: String,
    /// The symbols extracted from this file.
    pub symbols: Vec<Symbol>,
}

/// The Deterministic Code Graph: per-file symbol sets.
///
/// T28/T30 store the per-file nodes; cross-file edges (calls, imports,
/// inheritance) are added in a later task. `update_file` re-parses one file
/// and replaces its node — the incremental update path.
#[derive(Debug, Default)]
pub struct Dcg {
    /// The files in the graph (path -> FileNode).
    pub files: HashMap<PathBuf, FileNode>,
}

impl Dcg {
    /// Construct an empty DCG.
    pub fn new() -> Self {
        Self::default()
    }

    /// T30: re-parse one file and replace its FileNode. Other files are
    /// unchanged. Returns the new FileNode (or an error if parsing fails).
    pub fn update_file(&mut self, path: PathBuf, src: &str) -> anyhow::Result<FileNode> {
        let language = detect_language(&path);
        let symbols = match language.as_str() {
            "rust" => parse_rust(src)?,
            // Other languages land in later tasks; for now, no symbols.
            _ => vec![],
        };
        let node = FileNode {
            path: path.clone(),
            language,
            symbols,
        };
        self.files.insert(path, node.clone());
        Ok(node)
    }

    /// Get a file's node.
    pub fn get(&self, path: &std::path::Path) -> Option<&FileNode> {
        self.files.get(path)
    }

    /// All symbols across all files.
    pub fn all_symbols(&self) -> Vec<&Symbol> {
        self.files.values().flat_map(|f| f.symbols.iter()).collect()
    }

    /// The number of files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Detect the language from a file extension.
fn detect_language(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "rs" => "rust".into(),
        "py" => "python".into(),
        "ts" | "tsx" => "typescript".into(),
        "js" | "jsx" => "javascript".into(),
        "go" => "go".into(),
        "java" => "java".into(),
        "c" | "h" => "c".into(),
        "cpp" | "cc" | "cxx" | "hpp" => "cpp".into(),
        _ => "unknown".into(),
    }
}

/// T29: parse a Rust source string into a Vec<Symbol> using tree-sitter.
///
/// Extracts top-level + impl-level: `fn` (Function), `struct` (Struct),
/// `enum` (Enum), `impl` (Impl — and its `fn`s as Method), `trait` (Trait),
/// `mod` (Module), `const`/`static` (Constant), `type` (TypeAlias).
pub fn parse_rust(src: &str) -> anyhow::Result<Vec<Symbol>> {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into())?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter returned no tree for the Rust source"))?;
    let root = tree.root_node();
    let mut symbols = vec![];
    walk_rust(&root, src, &mut symbols, None);
    Ok(symbols)
}

/// Recursively walk the tree, collecting symbols.
///
/// `impl_owner` is the name of the type whose impl block we're inside (so its
/// `fn`s become Methods).
fn walk_rust(
    node: &tree_sitter::Node,
    src: &str,
    symbols: &mut Vec<Symbol>,
    impl_owner: Option<&str>,
) {
    // Recurse into named children.
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "function_item" | "function_signature_item" => {
                // Inside an impl or trait -> Method; else -> Function.
                let kind = if impl_owner.is_some() {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                };
                if let Some(sym) = make_symbol(&child, src, kind) {
                    symbols.push(sym);
                }
            }
            "struct_item" => {
                if let Some(sym) = make_symbol(&child, src, SymbolKind::Struct) {
                    symbols.push(sym);
                }
            }
            "enum_item" => {
                if let Some(sym) = make_symbol(&child, src, SymbolKind::Enum) {
                    symbols.push(sym);
                }
            }
            "trait_item" => {
                if let Some(sym) = make_symbol(&child, src, SymbolKind::Trait) {
                    symbols.push(sym);
                }
                // Recurse into the trait body, marking fn signatures as Methods.
                let trait_name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(src.as_bytes()).ok())
                    .map(|s| s.trim().to_string());
                walk_rust(&child, src, symbols, trait_name.as_deref());
                continue;
            }
            "type_item" => {
                if let Some(sym) = make_symbol(&child, src, SymbolKind::TypeAlias) {
                    symbols.push(sym);
                }
            }
            "const_item" | "static_item" => {
                if let Some(sym) = make_symbol(&child, src, SymbolKind::Constant) {
                    symbols.push(sym);
                }
            }
            "mod_item" => {
                if let Some(sym) = make_symbol(&child, src, SymbolKind::Module) {
                    symbols.push(sym);
                }
            }
            "impl_item" => {
                // The impl's "name" is the type it's for (the `type` field).
                let impl_name = child
                    .child_by_field_name("type")
                    .and_then(|t| t.utf8_text(src.as_bytes()).ok())
                    .map(|s| s.trim().to_string());
                if let Some(ref name) = impl_name {
                    symbols.push(Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Impl,
                        start: child.start_position().into(),
                        end: child.end_position().into(),
                    });
                    // Recurse into the impl's body, marking fn's as Methods.
                    walk_rust(&child, src, symbols, Some(name));
                } else {
                    // No type name (rare) — still recurse.
                    walk_rust(&child, src, symbols, None);
                }
                continue; // already recursed
            }
            _ => {
                // Recurse into other nodes (e.g. source_file, blocks) to find
                // nested symbols, but don't double-handle impl_item.
                walk_rust(&child, src, symbols, impl_owner);
                continue;
            }
        }
        // For leaf symbols (fn/struct/etc.), also recurse in case of nested
        // items (Rust allows nested fn/struct/impl).
        walk_rust(&child, src, symbols, None);
    }
}

/// Make a Symbol from a node, extracting the `name` field.
fn make_symbol(node: &tree_sitter::Node, src: &str, kind: SymbolKind) -> Option<Symbol> {
    let name = node.child_by_field_name("name")?;
    let name_text = name.utf8_text(src.as_bytes()).ok()?.trim().to_string();
    if name_text.is_empty() {
        return None;
    }
    Some(Symbol {
        name: name_text,
        kind,
        start: node.start_position().into(),
        end: node.end_position().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T28: Dcg constructs empty + update_file + get + all_symbols.
    #[test]
    fn dcg_constructs_and_updates() {
        let mut dcg = Dcg::new();
        assert_eq!(dcg.file_count(), 0);
        assert!(dcg.get(&PathBuf::from("a.rs")).is_none());

        let node = dcg
            .update_file(PathBuf::from("a.rs"), "fn foo() {}")
            .expect("parse");
        assert_eq!(node.language, "rust");
        assert_eq!(node.symbols.len(), 1);
        assert_eq!(node.symbols[0].name, "foo");
        assert_eq!(node.symbols[0].kind, SymbolKind::Function);

        assert_eq!(dcg.file_count(), 1);
        assert!(dcg.get(&PathBuf::from("a.rs")).is_some());
        assert_eq!(dcg.all_symbols().len(), 1);
    }

    /// T29: parse_rust extracts fn/struct/enum/impl + impl methods.
    #[test]
    fn parse_rust_extracts_symbols() {
        let src = r#"fn foo() {}
struct Bar { x: i32 }
enum Baz { A, B }
impl Bar {
    fn m(&self) {}
    fn n(&self) {}
}
trait T {
    fn t_fn(&self);
}
const C: i32 = 5;
type Alias = i32;
mod inner {}
"#;
        let symbols = parse_rust(src).expect("parse");
        // Count by kind (a name like "Bar" appears as both Struct + Impl, so a
        // by-name HashMap would dedupe + lose one).
        let count = |k: SymbolKind| symbols.iter().filter(|s| s.kind == k).count();

        assert_eq!(count(SymbolKind::Function), 1); // foo
        assert_eq!(count(SymbolKind::Struct), 1); // Bar
        assert_eq!(count(SymbolKind::Enum), 1); // Baz
        assert_eq!(count(SymbolKind::Trait), 1); // T
        assert_eq!(count(SymbolKind::Constant), 1); // C
        assert_eq!(count(SymbolKind::TypeAlias), 1); // Alias
        assert_eq!(count(SymbolKind::Module), 1); // inner

        // impl Bar -> an Impl symbol + two Methods (m, n).
        let impls: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Impl)
            .collect();
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].name, "Bar");
        let methods: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .map(|s| s.name.as_str())
            .collect();
        assert!(methods.contains(&"m"));
        assert!(methods.contains(&"n"));

        // The struct Bar exists (separate from the impl Bar).
        let structs: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Bar");
    }

    /// T29: positions are populated.
    #[test]
    fn parse_rust_positions_populated() {
        let src = r#"fn foo() {}
struct Bar { x: i32 }
enum Baz { A, B }
impl Bar { fn m(&self){} fn n(&self){} }
trait T { fn t_fn(&self); }
const C: i32 = 5;
type Alias = i32;
mod inner {}
"#;
        let symbols = parse_rust(src).expect("parse");
        let foo = symbols.iter().find(|s| s.name == "foo").unwrap();
        assert_eq!(foo.start.row, 0);
        assert_eq!(foo.end.row, 0); // foo is on line 0
        let bar = symbols.iter().find(|s| s.name == "Bar").unwrap();
        assert_eq!(bar.start.row, 1);
    }

    /// T30: update_file on an existing path replaces; other files unchanged.
    #[test]
    fn dcg_update_file_replaces() {
        let mut dcg = Dcg::new();
        dcg.update_file(PathBuf::from("a.rs"), "fn foo() {}")
            .unwrap();
        dcg.update_file(PathBuf::from("b.rs"), "fn bar() {}")
            .unwrap();
        assert_eq!(dcg.file_count(), 2);

        // Update a.rs with new content -> a's symbols reflect it, b unchanged.
        dcg.update_file(PathBuf::from("a.rs"), "fn baz() {}\nfn qux() {}")
            .unwrap();
        let a = dcg.get(&PathBuf::from("a.rs")).unwrap();
        let a_names: Vec<&str> = a.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(a_names.contains(&"baz"));
        assert!(a_names.contains(&"qux"));
        assert!(!a_names.contains(&"foo")); // old content replaced

        // b unchanged.
        let b = dcg.get(&PathBuf::from("b.rs")).unwrap();
        assert_eq!(b.symbols.len(), 1);
        assert_eq!(b.symbols[0].name, "bar");
    }

    /// T30: update_file detects the language from the extension.
    #[test]
    fn dcg_detects_language() {
        let mut dcg = Dcg::new();
        let n = dcg
            .update_file(PathBuf::from("x.py"), "def f(): pass")
            .unwrap();
        assert_eq!(n.language, "python");
        // No rust parser for .py -> no symbols (other-language parsers land later).
        assert!(n.symbols.is_empty());

        let n2 = dcg.update_file(PathBuf::from("x.rs"), "fn f() {}").unwrap();
        assert_eq!(n2.language, "rust");
        assert_eq!(n2.symbols.len(), 1);
    }
}
