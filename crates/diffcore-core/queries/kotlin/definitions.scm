; Kotlin — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.
;
; Note: the tree-sitter-kotlin AST uses bare `identifier` (not
; `simple_identifier` / `type_identifier`) for declaration names.

; Function declaration — fun foo() { ... }
(function_declaration
  (identifier) @name) @definition.function

; Class declaration — class Foo, data class Foo, sealed class Foo, ...
(class_declaration
  (identifier) @name) @definition.class

; Object declaration — object Foo { ... }
(object_declaration
  (identifier) @name) @definition.class

; Property declaration — val FOO = 42
(property_declaration
  (variable_declaration
    (identifier) @name)) @definition.property

; Type alias — typealias Foo = Bar
(type_alias
  (identifier) @name) @definition.typealias
