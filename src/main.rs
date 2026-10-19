use std::{env, error::Error, fs};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let path = args.next().ok_or("No input file provided, usage: luna-vm <filename>")?;
    let bytes = fs::read(&path)?;
    let chunk = luna_vm_::chunk::Chunk::parse(&bytes)?;
    let _ = chunk;

    Ok(())
}