; Rust — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Function declaration — fn foo() {}
(function_item
  name: (identifier) @name) @definition.function

; Struct definition — struct Foo {}
(struct_item
  name: (type_identifier) @name) @definition.struct

; Enum definition — enum Foo {}
(enum_item
  name: (type_identifier) @name) @definition.enum

; Trait definition — trait Foo {}
(trait_item
  name: (type_identifier) @name) @definition.trait

; Type alias — type Foo = Bar;
(type_item
  name: (type_identifier) @name) @definition.type_alias

; Const declaration — const FOO: i32 = 42;
(const_item
  name: (identifier) @name) @definition.constant

; Static declaration — static FOO: i32 = 42;
(static_item
  name: (identifier) @name) @definition.static

; Impl block method — impl Foo { fn bar() {} }
(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name) @definition.method))

; Macro definition — macro_rules! foo { ... }
(macro_definition
  name: (identifier) @name) @definition.macro
