; Swift — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.
;
; Note: tree-sitter-swift's `class_declaration` covers struct, class, enum,
; extension, and actor (the keyword is encoded as a child token, not as a
; distinct grammar node), so we tag it as `@definition.class` and accept
; the small loss of fidelity (struct/enum collapse to Class in our IR
; anyway).

; Function declaration — func foo() { ... }
(function_declaration
  name: (simple_identifier) @name) @definition.function

; Class / struct / enum / extension / actor — struct Foo { ... }
(class_declaration
  name: (type_identifier) @name) @definition.class

; Protocol declaration — protocol Foo { ... }
(protocol_declaration
  name: (type_identifier) @name) @definition.protocol

; Protocol function declaration — func foo() inside protocol body
(protocol_function_declaration
  name: (simple_identifier) @name) @definition.function

; Property declaration — let foo = 42
(property_declaration
  name: (pattern
    (simple_identifier) @name)) @definition.property

; Type alias — typealias Foo = Bar
(typealias_declaration
  name: (type_identifier) @name) @definition.typealias
