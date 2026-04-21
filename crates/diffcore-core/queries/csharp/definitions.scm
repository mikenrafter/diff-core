; C# — standard tree-sitter "tags" convention.
;
; See `queries/typescript/definitions.scm` for the convention writeup.

; Method declaration — public void Foo() {}
(method_declaration
  name: (identifier) @name) @definition.method

; Constructor declaration — public MyClass() {}
(constructor_declaration
  name: (identifier) @name) @definition.method

; Class declaration — public class Foo {}
(class_declaration
  name: (identifier) @name) @definition.class

; Struct declaration — public struct Foo {}
(struct_declaration
  name: (identifier) @name) @definition.struct

; Interface declaration — public interface IFoo {}
(interface_declaration
  name: (identifier) @name) @definition.interface

; Enum declaration — public enum Foo {}
(enum_declaration
  name: (identifier) @name) @definition.enum

; Record declaration — public record Foo {}
(record_declaration
  name: (identifier) @name) @definition.record

; Property declaration — public int Foo { get; set; }
(property_declaration
  name: (identifier) @name) @definition.property

; Field declaration — private int _foo;
(field_declaration
  (variable_declaration
    (variable_declarator
      name: (identifier) @name))) @definition.field

; Delegate declaration — public delegate void MyHandler(...);
(delegate_declaration
  name: (identifier) @name) @definition.delegate
