use icnow::parser::parse_file_in_memory;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_path = if args.len() > 1 { &args[1] } else { "/Users/cristian/projects/dgapp_bkp/app/controllers/users/sessions_controller.rb" };
    match parse_file_in_memory(file_path) {
        Ok((summary, nodes, edges)) => {
            println!("--- Summary ---");
            println!("{:#?}", summary);
            println!("--- Nodes ---");
            for node in nodes {
                println!("Node: {} ({})", node.0, node.2);
            }
        }
        Err(e) => {
            println!("Error parsing file: {}", e);
        }
    }
}
