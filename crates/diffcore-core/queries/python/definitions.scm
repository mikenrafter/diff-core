; Python — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for an overview of the convention
; (`@name` for the symbol identifier, `@definition.<kind>` for the symbol
; node). The standard-convention extractor in `query_engine.rs` deduplicates
; matches by the start byte of the `@name` capture, which means the bare
; (function_definition …) and the wrapping (decorated_definition …)
; patterns below correctly resolve to a single Definition each.

; Function definition — def foo(): ...
(function_definition
  name: (identifier) @name) @definition.function

; Class definition — class Foo: ...
(class_definition
  name: (identifier) @name) @definition.class

; Decorated function — @decorator\ndef foo(): ...
(decorated_definition
  definition: (function_definition
    name: (identifier) @name)) @definition.function

; Decorated class — @decorator\nclass Foo: ...
(decorated_definition
  definition: (class_definition
    name: (identifier) @name)) @definition.class

; Class method — class Foo: def bar(self): ...
(class_definition
  body: (block
    (function_definition
      name: (identifier) @name) @definition.method))

; Decorated class method — class Foo: @decorator\ndef bar(self): ...
(class_definition
  body: (block
    (decorated_definition
      definition: (function_definition
        name: (identifier) @name) @definition.method)))
