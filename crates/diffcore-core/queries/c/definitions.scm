; C — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Function definition — int foo(int x) { ... }
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

; Function definition with pointer return — int *foo() { ... }
(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

; Struct specifier — struct Foo { ... };
(struct_specifier
  name: (type_identifier) @name) @definition.struct

; Enum specifier — enum Color { RED, GREEN, BLUE };
(enum_specifier
  name: (type_identifier) @name) @definition.enum

; Union specifier — union Data { ... };
(union_specifier
  name: (type_identifier) @name) @definition.union

; Type definition — typedef int MyInt;
(type_definition
  declarator: (type_identifier) @name) @definition.type_alias

; Global variable declaration — int global_var = 42;
(declaration
  declarator: (init_declarator
    declarator: (identifier) @name)) @definition.variable
