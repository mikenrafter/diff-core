//! Smoke-tests for all 23 "extra" (post-core-13) language modules.
//!
//! Each test:
//!
//! 1. **Compilation smoke** — calls `parse_file` with an empty source string,
//!    which forces the [`QueryEngine`] to compile all four `.scm` files for
//!    that language.  If any query references a node type that does not exist
//!    in the grammar, `parse_file` returns a `QueryCompilation` error and the
//!    test panics with an informative message — far better than a silent empty
//!    outline at runtime.
//!
//! 2. **Basic definition extraction** — calls `parse_file` with a minimal but
//!    realistic source snippet and asserts that at least one expected symbol
//!    appears in `result.definitions`.
//!
//! The tests are grouped into feature-gated sub-modules so they compile only
//! when the corresponding `lang-*` Cargo feature is enabled (all are on by
//! default via the `extra-languages` feature).

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use crate::query_engine::shared_test_engine;

    fn e() -> &'static crate::query_engine::QueryEngine {
        shared_test_engine()
    }

    // ── helper ────────────────────────────────────────────────────────────

    /// Assert that parsing `source` as `ext` file:
    ///  (a) does not produce a query compilation error, and
    ///  (b) the parsed definitions contain a symbol named `expected_name`.
    fn assert_defines(ext: &str, source: &str, expected_name: &str) {
        let path = format!("test_file.{ext}");
        let result = e()
            .parse_file(&path, source)
            .unwrap_or_else(|e| panic!("{ext}: parse_file failed: {e}"));
        assert!(
            result.definitions.iter().any(|d| d.name == expected_name),
            "{ext}: expected symbol '{expected_name}' in definitions; got: {:?}",
            result
                .definitions
                .iter()
                .map(|d| &d.name)
                .collect::<Vec<_>>()
        );
    }

    /// Assert that parsing produces no compilation error (empty source is
    /// enough to trigger query compilation).
    fn assert_compiles(ext: &str) {
        let path = format!("test_file.{ext}");
        e().parse_file(&path, "")
            .unwrap_or_else(|e| panic!("{ext}: query compilation failed: {e}"));
    }

    // ── Bash ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-bash")]
    #[test]
    fn bash_compiles() {
        assert_compiles("sh");
    }

    #[cfg(feature = "lang-bash")]
    #[test]
    fn bash_function() {
        assert_defines("sh", "function greet() { echo hello; }", "greet");
    }

    // ── Haskell ───────────────────────────────────────────────────────────

    #[cfg(feature = "lang-haskell")]
    #[test]
    fn haskell_compiles() {
        assert_compiles("hs");
    }

    #[cfg(feature = "lang-haskell")]
    #[test]
    fn haskell_function() {
        assert_defines("hs", "add :: Int -> Int -> Int\nadd x y = x + y\n", "add");
    }

    // ── Nix ───────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-nix")]
    #[test]
    fn nix_compiles() {
        assert_compiles("nix");
    }

    #[cfg(feature = "lang-nix")]
    #[test]
    fn nix_function_binding() {
        assert_defines("nix", "{ foo = x: x + 1; }", "foo");
    }

    // ── Lua ───────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-lua")]
    #[test]
    fn lua_compiles() {
        assert_compiles("lua");
    }

    #[cfg(feature = "lang-lua")]
    #[test]
    fn lua_function() {
        assert_defines("lua", "function greet(name)\n  print('hi')\nend\n", "greet");
    }

    // ── Perl ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-perl")]
    #[test]
    fn perl_compiles() {
        assert_compiles("pl");
    }

    #[cfg(feature = "lang-perl")]
    #[test]
    fn perl_function() {
        assert_defines("pl", "sub greet { print 'hi'; }\n", "greet");
    }

    // ── Elixir ────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-elixir")]
    #[test]
    fn elixir_compiles() {
        assert_compiles("ex");
    }

    #[cfg(feature = "lang-elixir")]
    #[test]
    fn elixir_module() {
        assert_defines("ex", "defmodule MyApp do\nend\n", "MyApp");
    }

    #[cfg(feature = "lang-elixir")]
    #[test]
    fn elixir_function() {
        assert_defines(
            "ex",
            "defmodule M do\n  def greet(name), do: IO.puts(name)\nend\n",
            "greet",
        );
    }

    // ── Erlang ────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-erlang")]
    #[test]
    fn erlang_compiles() {
        assert_compiles("erl");
    }

    #[cfg(feature = "lang-erlang")]
    #[test]
    fn erlang_function() {
        assert_defines("erl", "greet() -> io:format(\"hi~n\").\n", "greet");
    }

    // ── Zig ───────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-zig")]
    #[test]
    fn zig_compiles() {
        assert_compiles("zig");
    }

    #[cfg(feature = "lang-zig")]
    #[test]
    fn zig_function() {
        assert_defines(
            "zig",
            "pub fn add(a: i32, b: i32) i32 { return a + b; }\n",
            "add",
        );
    }

    // ── OCaml ─────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-ocaml")]
    #[test]
    fn ocaml_compiles() {
        assert_compiles("ml");
    }

    #[cfg(feature = "lang-ocaml")]
    #[test]
    fn ocaml_function() {
        assert_defines("ml", "let add x y = x + y\n", "add");
    }

    // ── Julia ─────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-julia")]
    #[test]
    fn julia_compiles() {
        assert_compiles("jl");
    }

    #[cfg(feature = "lang-julia")]
    #[test]
    fn julia_function() {
        assert_defines("jl", "function add(x, y)\n  x + y\nend\n", "add");
    }

    // ── Dart ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-dart")]
    #[test]
    fn dart_compiles() {
        assert_compiles("dart");
    }

    #[cfg(feature = "lang-dart")]
    #[test]
    fn dart_class() {
        assert_defines(
            "dart",
            "class User {\n  final String name;\n  User(this.name);\n}\n",
            "User",
        );
    }

    #[cfg(feature = "lang-dart")]
    #[test]
    fn dart_top_level_function() {
        assert_defines("dart", "void main() { print('hi'); }\n", "main");
    }

    // ── R ─────────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-r")]
    #[test]
    fn r_compiles() {
        assert_compiles("r");
    }

    #[cfg(feature = "lang-r")]
    #[test]
    fn r_function() {
        assert_defines("r", "add <- function(x, y) x + y\n", "add");
    }

    // ── Fish ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-fish")]
    #[test]
    fn fish_compiles() {
        assert_compiles("fish");
    }

    #[cfg(feature = "lang-fish")]
    #[test]
    fn fish_function() {
        assert_defines("fish", "function greet\n  echo hi\nend\n", "greet");
    }

    // ── HTML ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-html")]
    #[test]
    fn html_compiles() {
        assert_compiles("html");
    }

    // ── CSS ───────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-css")]
    #[test]
    fn css_compiles() {
        assert_compiles("css");
    }

    #[cfg(feature = "lang-css")]
    #[test]
    fn css_class_selector() {
        // CSS class selectors: `.my-button { ... }` → the grammar emits just the
        // class name without the dot prefix (same as class_name node text).
        assert_defines("css", ".my-button { color: red; }\n", "my-button");
    }

    // ── SCSS ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-scss")]
    #[test]
    fn scss_compiles() {
        assert_compiles("scss");
    }

    #[cfg(feature = "lang-scss")]
    #[test]
    fn scss_mixin() {
        assert_defines(
            "scss",
            "@mixin flex-center {\n  display: flex;\n  align-items: center;\n}\n",
            "flex-center",
        );
    }

    // ── JSON ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-json")]
    #[test]
    fn json_compiles() {
        assert_compiles("json");
    }

    // ── YAML ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-yaml")]
    #[test]
    fn yaml_compiles() {
        assert_compiles("yaml");
    }

    // ── TOML ──────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-toml")]
    #[test]
    fn toml_compiles() {
        assert_compiles("toml");
    }

    // ── Markdown ──────────────────────────────────────────────────────────

    #[cfg(feature = "lang-markdown")]
    #[test]
    fn markdown_compiles() {
        assert_compiles("md");
    }

    // ── GraphQL ───────────────────────────────────────────────────────────

    #[cfg(feature = "lang-graphql")]
    #[test]
    fn graphql_compiles() {
        assert_compiles("graphql");
    }

    #[cfg(feature = "lang-graphql")]
    #[test]
    fn graphql_type() {
        assert_defines(
            "graphql",
            "type User {\n  id: ID!\n  name: String!\n}\n",
            "User",
        );
    }

    // ── Vue ───────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-vue")]
    #[test]
    fn vue_compiles() {
        assert_compiles("vue");
    }

    // ── Svelte ────────────────────────────────────────────────────────────

    #[cfg(feature = "lang-svelte")]
    #[test]
    fn svelte_compiles() {
        assert_compiles("svelte");
    }
}
