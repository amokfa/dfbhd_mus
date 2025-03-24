use std::path::PathBuf;
use std::str::FromStr;
use itertools::Itertools;
use dfbhd_mus::sbf::process_file;

fn main() {
    rayon::ThreadPoolBuilder::new().num_threads(16).build_global().unwrap();

    let mut game_dir = None;
    let mut output_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-dir" => {
                game_dir = Some(PathBuf::from_str(args.next().unwrap().as_str()).unwrap());
            }
            "--output-dir" => {
                output_dir = Some(PathBuf::from_str(args.next().unwrap().as_str()).unwrap());
            }
            _ => {
                println!("Unknown argument: {}", arg);
                std::process::exit(1);
            }
        }
    }
    let game_dir = game_dir.unwrap();
    let output_dir = output_dir.unwrap();
    let _ = std::fs::remove_dir_all(output_dir.join("wav"));
    std::fs::create_dir_all(output_dir.join("wav")).unwrap();
    let files = [
        game_dir.join("menumus.sbf"),
        game_dir.join("gamemus.sbf"),
        game_dir.join("EXP1.sbf")
    ];
    for file in files {
        if let Err(e) = process_file(file.as_path(), output_dir.as_path()) {
            dbg!(e);
        }
    }
}

