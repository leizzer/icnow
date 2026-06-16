use std::path::PathBuf;

fn main() {
    let db_path = "/Users/cristian/Projects/dg/dgapp/knowledge.db";
    // Ensure we start fresh
    let _ = std::fs::remove_dir_all(db_path);
    let _ = std::fs::remove_file(db_path);
    
    let root = "/Users/cristian/Projects/dg/dgapp";
    println!("Starting LSIF generation...");
    let lsif_path = format!("{}/dump.lsif", root);
    
    // Generate LSIF first
    let output = std::process::Command::new("solargraph")
        .arg("lsif")
        .current_dir(root)
        .output()
        .unwrap();
        
    if !output.status.success() {
        println!("Solargraph failed: {}", String::from_utf8_lossy(&output.stderr));
        return;
    }
    println!("LSIF generated at {}", lsif_path);
    
    println!("Importing LSIF...");
    match icnow::lsif::parse_and_import_lsif(&lsif_path, db_path, Some(root)) {
        Ok((nodes, edges)) => println!("Success! Nodes: {}, Edges: {}", nodes, edges),
        Err(e) => println!("Error during import: {}", e),
    }
}
