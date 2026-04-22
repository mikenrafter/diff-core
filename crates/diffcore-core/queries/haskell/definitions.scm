; Haskell — node types from tree-sitter-haskell 0.23 highlights.scm.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; Haskell uses `(decl ...)` as the top-level declaration node; the
; `name:` field holds a `(variable)` for functions and a `(name)` for
; type-level things (data, newtype, type alias, class).

; Top-level function / value binding — foo x = ...
(decl
  name: (variable) @name) @definition.function

; Type class declaration — class Eq a where ...
(decl/class
  name: (name) @name) @definition.class

; Data type declaration — data Maybe a = ...
(decl/data
  name: (name) @name) @definition.class

; Newtype declaration — newtype Wrapper a = ...
(decl/newtype
  name: (name) @name) @definition.class

; Type synonym — type Alias = ...
(decl/type
  name: (name) @name) @definition.type_alias
