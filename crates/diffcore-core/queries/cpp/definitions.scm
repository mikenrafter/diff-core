; C++ — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Function definition — int foo(int x) { ... }
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

; Function definition with pointer return — int* foo() { ... }
(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

; Function definition with qualified name — void MyClass::method() { ... }
(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @name))) @definition.method

; Class specifier — class Foo { ... };
(class_specifier
  name: (type_identifier) @name) @definition.class

; Struct specifier — struct Bar { ... };
(struct_specifier
  name: (type_identifier) @name) @definition.struct

; Enum specifier — enum Color { RED, GREEN };
(enum_specifier
  name: (type_identifier) @name) @definition.enum

; Type alias — using MyType = std::vector<int>;
(alias_declaration
  name: (type_identifier) @name) @definition.alias

; Namespace definition — namespace foo { ... }
; Mapped to SymbolKind::Module via the standard kind table.
(namespace_definition
  name: (namespace_identifier) @name) @definition.namespace

; Template-wrapped function definition
(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

; Template-wrapped class definition
(template_declaration
  (class_specifier
    name: (type_identifier) @name)) @definition.class
