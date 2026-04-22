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

; Custom block (e.g. <docs>) — treated as a module-level constant.
(component
  (start_tag
    (tag_name) @name)) @definition.module
