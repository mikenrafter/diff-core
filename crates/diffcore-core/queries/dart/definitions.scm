; Dart — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention overview.

; Class declaration — class Foo { ... }
(class_definition
  name: (identifier) @name) @definition.class

; Mixin declaration — mixin Foo { ... }
(mixin_declaration
  name: (identifier) @name) @definition.class

; Extension declaration — extension Foo on Bar { ... }
(extension_declaration
  name: (identifier) @name) @definition.class

; Enum declaration — enum Foo { ... }
(enum_declaration
  name: (identifier) @name) @definition.enum

; Function declaration — void foo() { ... }
(function_signature
  name: (identifier) @name) @definition.function

; Method declaration — class Foo { void bar() { ... } }
(method_signature
  name: (identifier) @name) @definition.method

; Getter — get foo => ...
(getter_signature
  name: (identifier) @name) @definition.method

; Setter — set foo(value) { ... }
(setter_signature
  name: (identifier) @name) @definition.method

; Constructor — Foo() { ... } or Foo.named() { ... }
(constructor_signature
  name: (identifier) @name) @definition.method

; Top-level variable — final foo = 42;
(top_level_definition
  (initialized_identifier_list
    (initialized_identifier
      (identifier) @name))) @definition.constant

; Type alias — typedef Foo = Bar;
(type_alias
  name: (identifier) @name) @definition.type_alias
