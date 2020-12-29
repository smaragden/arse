mod lib;
use std::env;
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("You must provide a path.")
    }
    let filename = args[1].as_str();
    println!("Filename: {}", filename);

    let parser = lib::ArseParser::new(lib::reader(filename));
    for node in parser {
        println!("{}", node.name);
    }
}
