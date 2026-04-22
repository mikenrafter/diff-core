; R — adapted from upstream tree-sitter-r tags.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; foo <- function(...) { ... }
(binary_operator
  lhs: (identifier) @name
  operator: "<-"
  rhs: (function_definition)) @definition.function

; foo = function(...) { ... }
(binary_operator
  lhs: (identifier) @name
  operator: "="
  rhs: (function_definition)) @definition.function
