fn main() {
    let db_path = "/tmp/knowledge.db";
    let graph = graphqlite::Graph::open(db_path).unwrap();
    let file_path = "/Users/cristian/Projects/dgapp_bkp/app/models/user.rb";
    println!("Parsing file {file_path}...");
    match icnow::parser::parse_file(file_path, &graph) {
        Ok(summary) => {
            println!("Parse succeeded!");
            println!("Found imports: {:?}", summary.imports);
            println!("Found structures: {:?}", summary.structures);
            println!(
                "Found standalone functions: {:?}",
                summary.standalone_functions
            );
            println!(
                "Found methods count: {}",
                summary.methods.values().map(|m| m.len()).sum::<usize>()
            );
            println!("Done");
        }
        Err(e) => println!("Parse failed: {e:?}"),
    }
}
