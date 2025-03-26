use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use itertools::Itertools;
use dfbhd_mus::sbf::{upscale_pcm, write_wav_header, SBFChunkData, SBF};
use rayon::prelude::*;
use dfbhd_mus::array_transmute;

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
    let tracks_config = serde_json::from_str::<HashMap<String, Vec<String>>>(std::fs::read_to_string("reordering_config.json").unwrap().as_str()).unwrap();
    let sbfs = [
        SBF::from_file(&game_dir.join("menumus.sbf")).unwrap(),
        SBF::from_file(&game_dir.join("gamemus.sbf")).unwrap(),
        SBF::from_file(&game_dir.join("EXP1.sbf")).unwrap(),
    ];

    let _ = std::fs::remove_dir_all(output_dir.join("wav"));
    std::fs::create_dir_all(output_dir.join("wav")).unwrap();
    tracks_config.par_iter()
        .for_each(|(track_name, chunks)| {
            let sbf = sbfs.iter()
                .find(|sbf| sbf.grouped_chunks.keys().contains(&track_name))
                .unwrap();
            let wav_path = output_dir.join("wav").join(format!("{track_name}.wav"));
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&wav_path)
                .unwrap();
            let chunks = chunks.iter()
                .map(|suffix| sbf.chunks.iter().find(|chunk| &chunk.ident == track_name && &chunk.suffix == suffix).unwrap())
                .collect::<Vec<_>>();
            let mut total_size = 0;
            for e in chunks.iter() {
                let segment_blob = &sbf.content[e.start as usize..(e.start + e.size) as usize];
                for chunk in array_transmute::<_, SBFChunkData>(segment_blob) {
                    total_size += chunk.size * 2;
                }
            }
            write_wav_header(&mut f, total_size).unwrap();
            let mut parsed_data = arrayvec::ArrayVec::<_, 8192>::new();
            for e in chunks.iter() {
                let segment_blob = &sbf.content[e.start as usize..(e.start + e.size) as usize];
                for chunk in array_transmute::<_, SBFChunkData>(segment_blob) {
                    parsed_data.clear();
                    for &b in &chunk.content[0..chunk.size as usize] {
                        let bytes = upscale_pcm(b, chunk.scale1).to_le_bytes();
                        parsed_data.extend(bytes);
                    }
                    f.write_all(parsed_data.as_slice()).unwrap();
                }
            }
            f.flush().unwrap();
        });
}

