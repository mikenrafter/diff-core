; GraphQL — node types from the tree-sitter-graphql grammar.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; GraphQL schemas are composed entirely of named type and directive
; definitions — every meaningful symbol in a schema file is a definition.

; Object type — type Foo { ... }
(object_type_definition
  name: (name) @name) @definition.class

; Interface type — interface Foo { ... }
(interface_type_definition
  name: (name) @name) @definition.interface

; Enum type — enum Foo { ... }
(enum_type_definition
  name: (name) @name) @definition.enum

; Union type — union Foo = A | B
(union_type_definition
  name: (name) @name) @definition.class

; Input object type — input Foo { ... }
(input_object_type_definition
  name: (name) @name) @definition.class

; Scalar type — scalar Foo
(scalar_type_definition
  name: (name) @name) @definition.type_alias

; Directive definition — directive @foo on ...
(directive_definition
  name: (name) @name) @definition.function

; Named operation — query Foo { ... } / mutation Foo { ... }
(operation_definition
  name: (name) @name) @definition.function

; Fragment — fragment Foo on Bar { ... }
(fragment_definition
  name: (name) @name) @definition.function
