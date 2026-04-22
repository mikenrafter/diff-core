; Elixir — adapted from upstream tree-sitter-elixir tags.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Module / protocol / behaviour definition
(call
  target: (identifier) @ignore
  (arguments (alias) @name)
  (#any-of? @ignore "defmodule" "defprotocol")) @definition.module

; Function / macro definition (def, defp, defmacro, …)
(call
  target: (identifier) @ignore
  (arguments
    [
      (identifier) @name
      (call target: (identifier) @name)
      (binary_operator
        left: (call target: (identifier) @name)
        operator: "when")
    ])
  (#any-of? @ignore "def" "defp" "defdelegate" "defguard" "defguardp" "defmacro" "defmacrop")) @definition.function
