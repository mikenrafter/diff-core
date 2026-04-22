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

; @layer name — @layer foo { ... }
(layer_statement
  (layer_name_list
    (dotted_name) @name)) @definition.module

; @counter-style name — @counter-style foo { ... }
(counter_style_rule
  (counter_style_name) @name) @definition.type_alias
