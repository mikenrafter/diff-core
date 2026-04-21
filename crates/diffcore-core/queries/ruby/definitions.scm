; Ruby — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Method definition — def foo() ... end
(method
  name: (identifier) @name) @definition.method

; Singleton method — def self.foo() ... end
(singleton_method
  name: (identifier) @name) @definition.method

; Class declaration — class Foo ... end / class Foo < Bar ... end
(class
  name: (constant) @name) @definition.class

; Module declaration — module Foo ... end
; Mapped to SymbolKind::Module via the standard kind table.
(module
  name: (constant) @name) @definition.module

; Constant assignment — FOO = 42
(assignment
  left: (constant) @name
  right: (_)) @definition.constant
