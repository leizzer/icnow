fn main() {
    tracing_subscriber::fmt::init();
    let project_root = std::path::Path::new("/Users/cristian/Projects/dgapp_bkp");
    let db_path = "/Users/cristian/Projects/dgapp_bkp/knowledge.db";
    println!("Running reconcile_workspace...");
    if let Err(e) = icnow::watcher::reconcile_workspace(project_root, db_path) {
        println!("Error: {e}");
    }
    println!("Finished.");
}
