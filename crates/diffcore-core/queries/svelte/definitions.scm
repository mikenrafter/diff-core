; Svelte Single-File Components — like Vue SFCs, the meaningful symbol
; definitions live inside the injected <script> content, which is parsed
; as TypeScript/JavaScript by the TS/JS query engine.
; See `queries/typescript/definitions.scm` for the convention overview.
;
; At the Svelte-grammar level there are no named symbol declarations;
; this file intentionally stays empty so the standard fallback extractor
; is used, which produces zero definitions (correct for a .svelte file
; treated as a template boundary rather than an implementation file).
