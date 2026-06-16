fn main() {
    let db_path = "/Users/cristian/projects/dgapp_bkp/knowledge.db";
    let root = "/Users/cristian/projects/dgapp_bkp";
    let lsif_path = format!("{}/dump.lsif", root);
    
    println!("Importing LSIF from {}...", lsif_path);
    match icnow::lsif::parse_and_import_lsif(&lsif_path, db_path, Some(root)) {
        Ok((nodes, edges)) => println!("Success! Nodes: {}, Edges: {}", nodes, edges),
        Err(e) => println!("Error during import: {}", e),
    }
}
