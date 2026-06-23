use icnow::parser::parse_file;
fn main() {
    let db_path = "/Users/cristian/Projects/dgapp_bkp/knowledge.db";
    let graph = icnow::open_db_graph(db_path).unwrap();
    let res = parse_file("/Users/cristian/Projects/dgapp_bkp/app/models/user.rb", &graph);
    println!("{:?}", res);
}
