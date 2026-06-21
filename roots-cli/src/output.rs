use serde::Serialize;

pub fn json<T: Serialize>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

pub fn error(message: &str) {
    eprintln!("{}", serde_json::json!({"error": message}));
}
