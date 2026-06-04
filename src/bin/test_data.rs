fn main() {
    let db_path = "knowledge_backup.db";
    let graph = graphqlite::Graph::open(db_path).unwrap();
    println!("Parsing against knowledge_backup.db:");
    match icnow::parser::parse_file(
        "/Users/cristian/Projects/dgapp_bkp/app/models/user.rb",
        &graph,
    ) {
        Ok(_) => println!("Parse succeeded!"),
        Err(e) => println!("Parse failed with: {e:?}"),
    }
}
