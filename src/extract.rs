use std::fs::File;
use std::io::Read;
use std::path::Path;
use triglyph::extract_trigrams;

/// Known text file extensions
const TEXT_EXTENSIONS: &[&str] = &[
    // Programming
    "rs", "py", "pyw", "pyi", "js", "mjs", "cjs", "ts", "mts", "cts", "jsx", "tsx",
    "go", "mod", "sum", "c", "h", "cpp", "hpp", "cc", "hh", "cxx", "hxx",
    "java", "kt", "kts", "scala", "groovy", "gradle",
    "rb", "rake", "gemspec", "php", "phtml",
    "pl", "pm", "pod", "sh", "bash", "zsh", "fish",
    "lua", "vim", "vimrc", "el", "lisp", "scm", "rkt",
    "clj", "cljs", "cljc", "edn", "hs", "lhs", "cabal",
    "ml", "mli", "ex", "exs", "eex", "erl", "hrl",
    "fs", "fsi", "fsx", "swift", "m", "mm", "r", "jl", "nim", "zig", "d", "cr", "dart", "elm",
    // Config
    "json", "jsonc", "json5", "yaml", "yml", "toml", "xml", "xsd", "xsl", "svg",
    "html", "htm", "xhtml", "css", "scss", "sass", "less",
    "ini", "cfg", "conf", "config", "env", "envrc", "properties",
    // Docs
    "md", "markdown", "mdown", "mkd", "rst", "adoc", "org", "txt", "text",
    "tex", "latex", "ltx",
    // Query/schema
    "sql", "ddl", "graphql", "gql", "prisma", "proto",
    // Build
    "dockerfile", "containerfile", "makefile", "mk", "cmake", "just", "ninja",
    "bazel", "bzl", "nix", "dhall",
    // Git
    "gitignore", "gitattributes", "gitmodules",
    // Misc
    "csv", "tsv", "log", "diff", "patch",
];

/// Known filenames to index
const KNOWN_FILENAMES: &[&str] = &[
    "Makefile", "makefile", "GNUmakefile", "Dockerfile", "Containerfile",
    "Gemfile", "Rakefile", "CMakeLists.txt",
    "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json",
    "go.mod", "go.sum", "requirements.txt", "Pipfile", "pyproject.toml",
    "setup.py", "setup.cfg", "composer.json", "pom.xml", "build.gradle",
    "BUILD", "BUILD.bazel", "WORKSPACE", "flake.nix", "shell.nix", "default.nix",
    "justfile", "Justfile", ".gitignore", ".gitattributes", ".editorconfig",
    "README", "README.md", "README.rst", "README.txt",
    "LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING",
    "CHANGELOG", "CHANGELOG.md", "AUTHORS", "CONTRIBUTORS",
];

pub const DEFAULT_MAX_SIZE: u64 = 1024 * 1024; // 1 MB

#[derive(Clone)]
pub struct ExtractConfig {
    pub max_file_size: u64,
    pub follow_symlinks: bool,
    pub respect_gitignore: bool,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_SIZE,
            follow_symlinks: false,
            respect_gitignore: true,
        }
    }
}

/// Check if a file should be indexed.
pub fn is_indexable(path: &Path, config: &ExtractConfig) -> bool {
    let meta = match if config.follow_symlinks {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    } {
        Ok(m) => m,
        Err(_) => return false,
    };

    if !meta.is_file() || meta.len() == 0 || meta.len() > config.max_file_size {
        return false;
    }

    // Check known filenames
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if KNOWN_FILENAMES.contains(&name) {
            return true;
        }
    }

    // Check extension
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        if TEXT_EXTENSIONS.contains(&ext_lower.as_str()) {
            return true;
        }
    }

    // Sniff content
    is_likely_text(path)
}

fn is_likely_text(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut buf = [0u8; 8192];
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return false,
    };

    !buf[..n].contains(&0)
}

/// Extract trigrams from a file.
pub fn extract_file_trigrams(path: &Path) -> Option<Vec<u32>> {
    let content = std::fs::read_to_string(path).ok()?;
    if content.len() < 3 {
        return None;
    }
    let trigrams = extract_trigrams(&content);
    if trigrams.is_empty() { None } else { Some(trigrams) }
}
