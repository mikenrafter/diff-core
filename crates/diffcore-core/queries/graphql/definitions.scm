; GraphQL — node types from the tree-sitter-graphql-0.1.0 grammar.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; IMPORTANT: all GraphQL definition nodes have `"fields": {}` in the
; tree-sitter-graphql grammar — no `name:` field labels exist. The
; `name` node type IS a named child, accessed positionally: `(node (name) @name)`.

; Object type — type Foo { ... }
(object_type_definition
  (name) @name) @definition.class

; Interface type — interface Foo { ... }
(interface_type_definition
  (name) @name) @definition.interface

; Enum type — enum Foo { ... }
(enum_type_definition
  (name) @name) @definition.enum

; Union type — union Foo = A | B
(union_type_definition
  (name) @name) @definition.class

; Input object type — input Foo { ... }
(input_object_type_definition
  (name) @name) @definition.class

; Scalar type — scalar Foo
(scalar_type_definition
  (name) @name) @definition.type_alias

; Directive definition — directive @foo on ...
(directive_definition
  (name) @name) @definition.function

; Named operation — query Foo { ... } / mutation Foo { ... }
(operation_definition
  (name) @name) @definition.function

; Fragment — fragment Foo on Bar { ... }
; NOTE: fragment_definition wraps its name in a `fragment_name` node;
; `(name)` is NOT a direct child of `fragment_definition`.
(fragment_definition
  (fragment_name
    (name) @name)) @definition.function
