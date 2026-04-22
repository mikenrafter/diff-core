; CSS — meaningful "definitions" are named selectors and at-rules.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; CSS has no symbol definitions in the traditional sense, but class
; selectors (.foo), id selectors (#foo), and named @keyframes / @layer
; / @counter-style blocks are the closest equivalent — they are what
; other files reference.

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

; @keyframes name — @keyframes foo { ... }
(keyframes_statement
  (keyframes_name) @name) @definition.function

; @layer — tree-sitter-css-0.25 does not expose `layer_statement` as a
; distinct named node; @layer rules are represented as `at_rule` which
; has no structured name child. Skip for now.
;
; @counter-style — tree-sitter-css-0.25 does not expose a named
; `counter_style_rule` node; @counter-style rules are represented as
; `at_rule` (generic) which has no structured name child. Skipped.
