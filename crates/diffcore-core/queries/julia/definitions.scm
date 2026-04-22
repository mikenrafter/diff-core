; Julia — node types from tree-sitter-julia 0.23 node-types.json.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; IMPORTANT: function_definition, macro_definition, struct_definition,
; abstract_definition, and primitive_definition all have `"fields": {}`
; in tree-sitter-julia 0.23 — none expose a `name:` field label.
; The function name lives inside a `signature` → `call_expression` →
; first-child `identifier` chain; struct/abstract/primitive names live
; inside a `type_head` → first-child `identifier` chain.

; Function definition — function foo(...) ... end
; The name lives as the first identifier inside the signature's call.
(function_definition
  (signature
    [
      (identifier) @name
      (call_expression . (identifier) @name)
    ])) @definition.function

; Macro definition — macro foo(...) ... end
(macro_definition
  (signature
    [
      (identifier) @name
      (call_expression . (identifier) @name)
    ])) @definition.function

; Struct — struct Foo ... end  /  mutable struct Foo ... end
; The name is the first identifier inside the type_head.
(struct_definition
  (type_head
    (identifier) @name)) @definition.class

; Abstract type — abstract type Foo end
(abstract_definition
  (type_head
    (identifier) @name)) @definition.class

; Primitive type — primitive type Foo N end
(primitive_definition
  (type_head
    (identifier) @name)) @definition.class

; Module — module Foo ... end
; module_definition is the ONE Julia definition node with a `name:` field.
(module_definition
  name: (identifier) @name) @definition.module
