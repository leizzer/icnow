use std::process::{Command, Stdio};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

fn main() {
    let mut f = OpenOptions::new().create(true).append(true).open("/Users/cristian/Projects/blackhole/icnow/wrapper.log").unwrap();
    writeln!(f, "Wrapper launched! Args: {:?}", env::args().collect::<Vec<_>>()).unwrap();
    
    let stderr_file = OpenOptions::new().create(true).append(true).open("/Users/cristian/Projects/blackhole/icnow/wrapper_stderr.log").unwrap();

    let status = Command::new("/Users/cristian/Projects/blackhole/icnow/target/release/icnow")
        .args(env::args().skip(1))
        .stderr(Stdio::from(stderr_file))
        .status()
        .unwrap();
    
    writeln!(f, "icnow exited with status: {}", status).unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
