; Fish shell — node types from tree-sitter-fish grammar.
; See `queries/typescript/definitions.scm` for the convention overview.

; function foo … end
(function_definition name: (_) @name) @definition.function
