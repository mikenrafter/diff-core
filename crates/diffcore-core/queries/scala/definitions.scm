; Scala — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.
;
; AST node types in tree-sitter-scala:
;   function_definition  — def foo() = ...
;   function_declaration — def foo(): Type   (abstract; no body)
;   class_definition     — class Foo { ... } / case class Foo(...)
;   trait_definition     — trait Foo { ... }
;   object_definition    — object Foo { ... }
;   val_definition       — val x = ...
;   var_definition       — var x = ...
;   type_definition      — type Foo = Bar

; Function definition — def foo() = expr
(function_definition
  (identifier) @name) @definition.function

; Function declaration (abstract) — def foo(): Type
(function_declaration
  (identifier) @name) @definition.function

; Class definition — class Foo / case class Foo(...)
(class_definition
  (identifier) @name) @definition.class

; Trait definition — trait Foo / sealed trait Foo
(trait_definition
  (identifier) @name) @definition.trait

; Object definition — object Foo { ... }
(object_definition
  (identifier) @name) @definition.class

; Val definition — val x = 42
(val_definition
  (identifier) @name) @definition.constant

; Var definition — var x = 42
(var_definition
  (identifier) @name) @definition.variable

; Type alias — type Foo = Bar
(type_definition
  (type_identifier) @name) @definition.type_alias
