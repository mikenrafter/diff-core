; Dart call sites — standard convention using @callee + @node captures.

; Function/method call — foo() or obj.foo()
(function_call_expression
  function: (_) @callee) @node

; Constructor call — new Foo() or Foo()
(instance_creation_expression
  constructor: (_) @callee) @node
