; Zig — node types from tree-sitter-zig-1.1.2 grammar.js.
; See `queries/typescript/definitions.scm` for the convention overview.

; Function declaration — pub fn foo(…) ReturnType { … }
; The name field is on the inner _function_prototype, which tree-sitter
; surfaces as a direct child; we anchor on function_declaration and
; match its descendent identifier via the name field.
(function_declaration
  name: (identifier) @name) @definition.function

; Struct definition assigned to a const — const Foo = struct { … };
; NOTE: variable_declaration has no `value:` field; the struct literal
; sits as a positional child after the `=` token.
(variable_declaration
  (identifier) @name
  "="
  (struct_declaration)) @definition.class

; Enum definition assigned to a const — const Foo = enum { … };
(variable_declaration
  (identifier) @name
  "="
  (enum_declaration)) @definition.enum

; Union definition assigned to a const — const Foo = union { … };
(variable_declaration
  (identifier) @name
  "="
  (union_declaration)) @definition.class

; Test block — test "description" { … }
; NOTE: tree-sitter-zig uses `(string)` not `(string_literal)` for the
; test name token. test_declaration has no `name:` field — string is
; a positional child.
(test_declaration
  (string) @name) @definition.function
