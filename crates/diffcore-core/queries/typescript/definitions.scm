; TypeScript / JavaScript — standard tree-sitter "tags" convention.
;
; Captures follow the universal-ctags / nvim-treesitter / GitHub code-nav
; convention so that `query_engine::extract_definitions_standard` can pick
; them up and so that this file is recognisable to anyone familiar with
; upstream `tree-sitter-typescript/queries/tags.scm`.
;
; Conventions:
;   - Every match has exactly one `@name` capture for the symbol identifier.
;   - The whole symbol node is tagged `@definition.<kind>`. Suffix maps to
;     SymbolKind via `standard_kind_to_symbol_kind`.

; Function declaration — function foo() { ... }
(function_declaration
  name: (identifier) @name) @definition.function

; Generator function declaration — function* foo() { ... }
(generator_function_declaration
  name: (identifier) @name) @definition.function

; Class declaration — class Foo { ... }
(class_declaration
  name: (_) @name) @definition.class

; Abstract class declaration — abstract class Foo { ... }
(abstract_class_declaration
  name: (_) @name) @definition.class

; Interface declaration — interface Foo { ... }
(interface_declaration
  name: (_) @name) @definition.interface

; Type alias declaration — type Foo = ...
(type_alias_declaration
  name: (_) @name) @definition.type_alias

; Variable declarator with arrow-function value — const foo = () => {}
(variable_declarator
  name: (identifier) @name
  value: (arrow_function)) @definition.function

; Variable declarator with function-expression value — const foo = function() {}
(variable_declarator
  name: (identifier) @name
  value: (function_expression)) @definition.function

; Variable declarator with non-function value — const FOO = 42
; The structural patterns above for arrow_function / function_expression run
; first; this fallback covers everything else (literals, calls, expressions).
(variable_declarator
  name: (identifier) @name
  value: [
    (number)
    (string)
    (template_string)
    (true)
    (false)
    (null)
    (object)
    (array)
    (regex)
    (call_expression)
    (member_expression)
    (identifier)
    (binary_expression)
    (unary_expression)
    (new_expression)
    (parenthesized_expression)
    (await_expression)
    (yield_expression)
    (subscript_expression)
    (as_expression)
    (satisfies_expression)
    (ternary_expression)
  ]) @definition.constant

; Class method — class Foo { bar() { ... } }
(method_definition
  name: (_) @name) @definition.method
