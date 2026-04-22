; Julia — node types from tree-sitter-julia 0.23 highlights.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Function definition — function foo(...) ... end  or  foo(x) = x
(function_definition
  name: (_) @name) @definition.function

; Macro definition — macro foo(...) ... end
(macro_definition
  name: (identifier) @name) @definition.function

; Struct — struct Foo ... end  /  mutable struct Foo ... end
(struct_definition
  name: (identifier) @name) @definition.class

; Abstract type — abstract type Foo end
(abstract_definition
  name: (identifier) @name) @definition.class

; Primitive type — primitive type Foo N end
(primitive_definition
  name: (identifier) @name) @definition.class

; Module — module Foo ... end
(module_definition
  name: (identifier) @name) @definition.module

; Type alias — Foo = Bar  (captured via type-head in parametrised forms)
(type_head (_) @name) @definition.type_alias
