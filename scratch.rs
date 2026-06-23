use kuzu::{Database, SystemConfig};
fn main() {
    let mut config = SystemConfig::default();
    config.read_only = true;
    // let db = Database::new("test.db", config);
}
