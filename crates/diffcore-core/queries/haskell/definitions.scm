; Haskell — node types from tree-sitter-haskell 0.23 highlights.scm.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; Haskell uses `(decl ...)` as the top-level declaration node; the
; `name:` field holds a `(variable)` for functions and a `(name)` for
; type-level things (data, newtype, type alias, class).

; Top-level function / value binding — foo x = ...
(decl
  name: (variable) @name) @definition.function

; Data / newtype / type-class declarations are complex in Haskell's
; tree-sitter grammar: the `name` lives inside a `type_head` child rather
; than on a `name:` field of the declaration node itself.
; Until we can verify the exact positional patterns against the grammar,
; these are intentionally omitted — function definitions (below) are the
; highest-value symbols for code review, and function extraction works
; correctly with the `(decl name:…)` pattern.
;
; TODO: add patterns for data_type / newtype / class_decl once the
; correct positional child paths have been tested.
