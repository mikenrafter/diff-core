; Vue Single-File Components — injected script content is parsed by the
; TypeScript/JavaScript query engine. This file captures the SFC-level
; structure visible to the tree-sitter-vue grammar.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; Note: `<script setup>` is treated as a single module; symbol extraction
; from the embedded script happens via the TS/JS injection, not here.

; Named component options — export default { name: "Foo", ... }
; The component name is declared inline via the `name` property or inferred
; from the file name. The best proxy at SFC level is the `<template>` or
; `<script>` element itself, but there are no named captures in the Vue
; grammar for component names. This file intentionally stays minimal;
; TS extraction from the injected `<script>` section handles definitions.

; The tree-sitter-vue-next grammar does not expose a `component` node;
; SFC-level structure uses `element`, `script_element`, `template_element`
; — but none carry a name that maps cleanly to our IR.
; Symbol extraction for Vue relies on the injected TS/JS parser for
; the `<script setup>` block rather than the SFC grammar itself.
