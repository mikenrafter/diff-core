; OCaml — simplified from upstream tree-sitter-ocaml tags.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Module definition — module Foo = ...
(module_definition
  (module_binding (module_name) @name)) @definition.module

; Module type / interface definition — module type Foo = ...
(module_type_definition (module_type_name) @name) @definition.interface

; Class definition
(class_definition
  (class_binding (class_name) @name)) @definition.class

; Type definition — type foo = ...
(type_definition
  (type_binding
    name: [
      (type_constructor) @name
      (type_constructor_path (type_constructor) @name)
    ])) @definition.type

; Function/value definition (let with parameter = function body)
(value_definition
  [
    (let_binding pattern: (value_name) @name (parameter))
    (let_binding
      pattern: (value_name) @name
      body: [(fun_expression) (function_expression)])
  ]) @definition.function
