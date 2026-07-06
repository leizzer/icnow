fn main() {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let mut icnow_dir = home.join(".icnow").join("db");
    let path = "test_parse_rust.db";
    let p = icnow_dir.join(std::path::Path::new(path).file_name().unwrap_or_else(|| std::ffi::OsStr::new("knowledge.db"))).to_string_lossy().to_string();
    println!("{}", p);
}
