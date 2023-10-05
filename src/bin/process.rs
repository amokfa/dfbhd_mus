use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use anyhow::{Result, Context};
use itertools::Itertools;
use dfbhd_mus::*;
use rayon::prelude::*;

fn main() {
    rayon::ThreadPoolBuilder::new().num_threads(32).build_global().unwrap();

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
    let _ = std::fs::remove_dir_all(&output_dir);
    std::fs::create_dir_all(&output_dir).unwrap();
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

fn process_file(file: &Path, output: &Path) -> Result<()> {
    let file = unsafe { memmap::Mmap::map(&File::open(file).context(format!("couldn't open {file:?}"))?) }?;
    let content = &file[..];
    let header: &[SBFHeader] = array_transmute(&content[0..size_of::<SBFHeader>()]);
    let index: &[SBFIndexEntryBin] = array_transmute(&content[header[0].index_offset as usize..(header[0].index_offset as usize + header[0].index_count as usize * size_of::<SBFIndexEntryBin>())]);
    index.iter().for_each(|e| {
        // assert_eq!(e.z1, 0); // non zero for only one chunk (VALIANT0)
        assert_eq!(e.z2, 0);
        assert_eq!(e.z3, 0);
        assert_eq!(e.block_size, 4104);
        assert_eq!(e.size % e.block_size, 0);
    });
    let index_parsed = index
        .iter()
        .map(|index| {
            let (ident, suffix) = if index.ident[0] == 'm' as u8 {
                let split_point = index.ident.iter().take_while(|&&b| b != 'a' as u8).count();
                (
                    index.ident.iter().take(split_point).map(|b| b.to_owned()).collect::<Vec<_>>(),
                    index.ident.iter().skip(split_point).take_while(|&&b| b != 0).map(|b| b.to_owned()).collect::<Vec<_>>()
                )
            } else {
                let split_point = index.ident.iter().take_while(|b| b.is_ascii_alphabetic()).count();
                (
                 index.ident.iter().take(split_point).map(|b| b.to_owned()).collect::<Vec<_>>(),
                 index.ident.iter().skip(split_point).take_while(|&&b| b != 0).map(|b| b.to_owned()).collect::<Vec<_>>()
                )
            };
            SBFIndexEntry {
                ident: String::from_utf8_lossy(ident.as_slice()).to_string(),
                suffix: String::from_utf8_lossy(suffix.as_slice()).to_string(),
                z1: index.z1,
                z2: index.z2,
                start: index.start,
                size: index.size,
                block_size: index.block_size,
                z3: index.z3,
            }
        })
        .collect::<Vec<_>>();

    let grouping = index_parsed.into_iter()
        .group_by(|index| index.ident.to_owned())
        .into_iter()
        .map(|(key, group)| (key, group.collect::<Vec<_>>()))
        .collect::<HashMap<_, _>>();
    grouping.par_iter().for_each(|(prefix, es)| {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(output.join(format!("{prefix}.raw")))
            .unwrap();
        es.iter().map(|e| {
            process_chunk(&content[e.start as usize..(e.start + e.size) as usize]).unwrap()
        })
            .for_each(|data| {
                f.write_all(data.as_slice()).unwrap();
                f.flush().unwrap();
            });
    });
    Ok(())
}

fn process_chunk(chunk: &[u8]) -> Result<Vec<u8>> {
    let mut parsed_data = vec![];
    for chunk in array_transmute::<_, SBFChunkData>(chunk) {
        assert_eq!(chunk.scale1, chunk.scale2);
        for &b in &chunk.content[0..chunk.size as usize] {
            let bytes = upscale_pcm(b, chunk.scale1).to_le_bytes();
            parsed_data.extend(bytes);
        }
    }
    Ok(parsed_data)
}

fn upscale_pcm(b: u8, scale: u8) -> i16 {
    let b = b as f32 - 127.5;
    let b = b / 140.0;
    let b = b * i16::MAX as f32;
    let b = b as i16 / ((scale as i16).pow(2) + 1);
    b / 2
}

#[repr(C)]
#[derive(Debug)]
struct SBFHeader {
    magic: [u8; 4],
    i1: u32,
    i2: u32,
    i3: u32,
    index_offset: u32,
    index_count: u32
}

#[repr(C)]
#[derive(Debug)]
struct SBFIndexEntryBin {
    ident: [u8; 8],
    z1: u32,
    z2: u32,
    start: u32,
    size: u32,
    block_size: u32,
    z3: u32,
}

#[derive(Debug, Clone)]
struct SBFIndexEntry {
    ident: String,
    suffix: String,
    z1: u32,
    z2: u32,
    start: u32,
    size: u32,
    block_size: u32,
    z3: u32,
}

#[repr(C)]
#[derive(Debug)]
struct SBFChunkData {
    size: u32,
    scale1: u8,
    scale2: u8,
    two_fifty: u8,
    zero: u8,
    content: [u8; 4096]
}