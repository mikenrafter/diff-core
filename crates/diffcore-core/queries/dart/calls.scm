; Dart call sites.
;
; The tree-sitter-dart grammar (orchard fork) does not expose a single
; "call_expression" node; calls are represented as an (identifier) sibling
; to a selector (argument_part), which is hard to capture cleanly as a
; single match. Call-site extraction for Dart is deferred until the grammar
; exposes a cleaner call node — leaving this file empty compiles to zero
; matches, which is the same observable behaviour as having no extraction.
