; Zig — node types from tree-sitter-zig-1.1.2 grammar.js.
; See `queries/typescript/definitions.scm` for the convention overview.

; Function declaration — pub fn foo(…) ReturnType { … }
; The name field is on the inner _function_prototype, which tree-sitter
; surfaces as a direct child; we anchor on function_declaration and
; match its descendent identifier via the name field.
(function_declaration
  name: (identifier) @name) @definition.function

; Struct definition assigned to a const — const Foo = struct { … };
(variable_declaration
  (identifier) @name
  value: (struct_declaration)) @definition.class

; Enum definition assigned to a const — const Foo = enum { … };
(variable_declaration
  (identifier) @name
  value: (enum_declaration)) @definition.enum

; Union definition assigned to a const — const Foo = union { … };
(variable_declaration
  (identifier) @name
  value: (union_declaration)) @definition.class

; Test block — test "description" { … }
(test_declaration
  name: (string_literal) @name) @definition.function
