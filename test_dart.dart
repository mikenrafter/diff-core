class User {
  final String name;
  User(this.name);
  
  void greet() {
    print("Hello, $name!");
  }
}

void main() {
  final user = User("Alice");
  user.greet();
}