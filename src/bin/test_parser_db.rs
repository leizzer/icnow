use icnow::parser::parse_file;

fn main() {
    let db_path = "test_parser_db.db";
    let _ = std::fs::remove_file(db_path);
    let conn = icnow::open_db_connection(db_path).unwrap();
    let file_path = "/Users/cristian/Projects/dgapp_bkp/app/javascript/theme/HostTheme/useHostContext.tsx";
    match parse_file(file_path, &conn) {
        Ok(summary) => {
            println!("Success! {:#?}", summary);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
