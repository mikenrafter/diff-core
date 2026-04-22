; Dart — based on upstream tree-sitter-dart-orchard tags.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Class declaration — class Foo { ... }
(class_definition
  name: (identifier) @name) @definition.class

; Mixin declaration — mixin Foo { ... }
; NOTE: no `name:` field; the first identifier child IS the name.
(mixin_declaration
  (mixin)
  (identifier) @name) @definition.class

; Extension declaration — extension Foo on Bar { ... }
(extension_declaration
  name: (identifier) @name) @definition.class

; Enum declaration — enum Foo { ... }
(enum_declaration
  name: (identifier) @name) @definition.enum

; Top-level function signature — void foo(...) { ... }
(function_signature
  name: (identifier) @name) @definition.function

; Method inside a class — void bar() { ... }
(method_signature
  (function_signature
    name: (identifier) @name)) @definition.method

; Getter — get foo => ...
(method_signature
  (getter_signature
    name: (identifier) @name)) @definition.method

; Setter — set foo(value) { ... }
(method_signature
  (setter_signature
    name: (identifier) @name)) @definition.method

; Constructor — Foo.named() { ... }
(method_signature
  (constructor_signature
    name: (identifier) @name)) @definition.method

; Factory constructor — factory Foo.named() { ... }
(method_signature
  (factory_constructor_signature
    (identifier) @name)) @definition.method

; Type alias — typedef Foo = Bar;
; NOTE: uses type_identifier, not plain identifier
(type_alias
  (type_identifier) @name) @definition.type_alias
