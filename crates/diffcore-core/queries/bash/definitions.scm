; Bash — node types from tree-sitter-bash grammar.
; See `queries/typescript/definitions.scm` for the convention overview.

; Both POSIX and Bash-style function definitions:
;   function foo() { … }
;   foo() { … }
(function_definition name: (word) @name) @definition.function
