use std::collections::HashMap;

struct User {
    name: String,
}

impl User {
    fn new(name: String) -> Self {
        User { name }
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

fn main() {
    let u = User::new("Alice".to_string());
    println!("{}", u.get_name());
}
