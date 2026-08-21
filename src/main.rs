fn main() {
    println!("Hello, world! {}", greeting());
}

fn greeting() -> String {
    "Hello, world!".to_string()
}

#[test]
fn test_greeting() {
    assert_eq!(greeting(), "Hello, world!");
}