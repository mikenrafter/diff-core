; PHP — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Method declaration — public function foo() {}
(method_declaration
  name: (name) @name) @definition.method

; Function definition — function foo() {}
(function_definition
  name: (name) @name) @definition.function

; Class declaration — class Foo {}
(class_declaration
  name: (name) @name) @definition.class

; Interface declaration — interface IFoo {}
(interface_declaration
  name: (name) @name) @definition.interface

; Trait declaration — trait Foo {}
(trait_declaration
  name: (name) @name) @definition.trait

; Enum declaration — enum Foo {}
(enum_declaration
  name: (name) @name) @definition.enum

; Constant declaration — const FOO = 42;
(const_declaration
  (const_element
    (name) @name)) @definition.constant
