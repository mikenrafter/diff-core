; SCSS — node types from the tree-sitter-scss grammar.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; SCSS adds three named-symbol concepts on top of plain CSS: mixins,
; functions, and placeholder selectors. Class/ID selectors are inherited
; from CSS semantics.

; Mixin definition — @mixin foo(...) { ... }
(mixin_statement
  name: (identifier) @name) @definition.function

; Function definition — @function foo(...) { ... }
(function_statement
  name: (identifier) @name) @definition.function

; Placeholder selector — %foo { ... }   (extended with @extend %foo)
; NOTE: tree-sitter-scss-1.0.0 uses `placeholder` as the node type
; for `%foo` selectors, but the name identifier inside it is not a
; standard named child and causes an "Impossible pattern" error.
; Skipped until the correct sub-node path is confirmed against the grammar.
;
; Class selector — .foo { ... }
(rule_set
  (selectors
    (class_selector
      (class_name) @name))) @definition.class

; ID selector — #foo { ... }
(rule_set
  (selectors
    (id_selector
      (id_name) @name))) @definition.constant
