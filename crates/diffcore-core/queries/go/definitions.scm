; Go — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the full convention writeup.
;
; Pattern declaration order matters: tree-sitter iterates matches in pattern
; order, and the standard-convention extractor in `query_engine.rs` dedups
; by `@name` start byte and keeps only the first match per name. So the
; struct/interface patterns must appear BEFORE the bare type-alias pattern
; — `type Foo struct{}` matches both, and we want SymbolKind::Class to win
; over SymbolKind::TypeAlias for the same source position.

; Function declaration — func Foo() {}
(function_declaration
  name: (identifier) @name) @definition.function

; Method declaration — func (r *Receiver) Foo() {}
(method_declaration
  name: (field_identifier) @name) @definition.method

; Type declaration with struct — type Foo struct {}
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (struct_type))) @definition.struct

; Type declaration with interface — type Foo interface {}
(type_declaration
  (type_spec
    name: (type_identifier) @name
    type: (interface_type))) @definition.interface

; Type alias / other type declaration — type Foo = Bar, type Foo int
; (Bare pattern; the struct/interface specialisations above run first.)
(type_declaration
  (type_spec
    name: (type_identifier) @name)) @definition.type_alias

; Const declaration — const Foo = 42
(const_declaration
  (const_spec
    name: (identifier) @name)) @definition.constant

; Var declaration — var foo int
(var_declaration
  (var_spec
    name: (identifier) @name)) @definition.variable
