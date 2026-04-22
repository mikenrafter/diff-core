; Nix — adapted from upstream tree-sitter-nix tags.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Top-level attribute binding that holds a function — foo = arg: ...
(binding
  attrpath: (attrpath attr: (identifier) @name)
  expression: (function_expression)) @definition.function
