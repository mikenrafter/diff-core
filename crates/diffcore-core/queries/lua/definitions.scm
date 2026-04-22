; Lua — adapted from upstream tree-sitter-lua tags.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Named function declaration — function foo() ... end
(function_declaration
  name: [
    (identifier) @name
    (dot_index_expression field: (identifier) @name)
  ]) @definition.function

; Method declaration — function MyClass:method() ... end
(function_declaration
  name: (method_index_expression method: (identifier) @name)) @definition.method

; Function assigned to variable — local foo = function() ... end
(assignment_statement
  (variable_list . name: [
    (identifier) @name
    (dot_index_expression field: (identifier) @name)
  ])
  (expression_list . value: (function_definition))) @definition.function

; Function field in table — { foo = function() ... end }
(table_constructor
  (field name: (identifier) @name value: (function_definition))) @definition.function
