//! CLI mirroring the Java oracle: read ELK JSON, run layout, print JSON.

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: elkrs <graph.json | ->");
        std::process::exit(2);
    }
    let input = if args[1] == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).expect("read stdin");
        buf
    } else {
        std::fs::read_to_string(&args[1]).expect("read input file")
    };

    let elk = elkrs::create_elk();
    match elk.layout_json(&input) {
        Ok(json) => println!("{}", serde_json::to_string_pretty(&json).unwrap()),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
