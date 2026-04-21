; Java — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Method declaration — public void foo() {}
(method_declaration
  name: (identifier) @name) @definition.method

; Constructor declaration — public Foo() {}
(constructor_declaration
  name: (identifier) @name) @definition.method

; Class declaration — public class Foo {}
(class_declaration
  name: (identifier) @name) @definition.class

; Interface declaration — public interface Foo {}
(interface_declaration
  name: (identifier) @name) @definition.interface

; Enum declaration — public enum Foo {}
(enum_declaration
  name: (identifier) @name) @definition.enum

; Annotation type declaration — public @interface Foo {}
(annotation_type_declaration
  name: (identifier) @name) @definition.annotation

; Field declaration — private int foo;
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @name)) @definition.field
