; Erlang — node types from tree-sitter-erlang highlights.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Function clause — foo(Args) -> ... .
; Every clause of a function shares the same atom name, so duplicates
; are deduped by the engine's seen_names set.
(function_clause name: (atom) @name) @definition.function

; Type definition — -type foo() :: ...
(type_name name: (atom) @name) @definition.type

; Record definition — -record(foo, {...}).
(record_decl name: (atom) @name) @definition.class

; Module attribute — -module(foo).
(module_attribute name: (atom) @name) @definition.module
