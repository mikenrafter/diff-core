; Perl — node types from tree-sitter-perl-next highlights.scm.
; See `queries/typescript/definitions.scm` for the convention overview.

; Subroutine declaration — sub foo { … }
(subroutine_declaration_statement name: (bareword) @name) @definition.function

; Method declaration — method foo { … }  (Moose / Object::Pad style)
(method_declaration_statement name: (bareword) @name) @definition.method

; Package declaration — package Foo;
(package_statement (package) @name) @definition.module

; Class declaration — class Foo { … }  (Object::Pad)
(class_statement (package) @name) @definition.class
