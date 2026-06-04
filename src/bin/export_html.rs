use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <db_path> <output.html> [filter_path]", args[0]);
        std::process::exit(1);
    }

    let db_path = &args[1];
    let out_path = &args[2];
    let filter_path = if args.len() > 3 {
        args[3].clone()
    } else {
        "".to_string()
    };

    println!("Connecting to database at {db_path}...");
    println!("Exporting graph...");
    icnow::exporter::generate_html(db_path, out_path, &filter_path)?;
    println!("Export completed successfully to {out_path}!");

    Ok(())
}
