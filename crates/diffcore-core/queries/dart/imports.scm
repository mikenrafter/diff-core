; Dart import patterns — used by extract_minimal_imports.
;
; The actual AST path for `import 'dart:core';` is:
;   library_import
;     import_specification
;       configurable_uri        ← NOT plain `uri`
;         uri
;           string_literal      ← the path string
;
; extract_minimal_imports strips surrounding quotes from @source.

; Regular import — import 'dart:core';
(import_specification
  (configurable_uri
    (uri
      (string_literal) @source))) @stmt

