use std::fs::File;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use anyhow::{Result, Context};
use dfbhd_mus::*;

fn main() {
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
    let files = [
        // game_dir.join("menumus.sbf"),
        game_dir.join("gamemus.sbf"),
        // game_dir.join("EXP1.sbf")
    ];
    for file in files {
        process_file(file.as_path(), output_dir.as_path());
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
        assert_eq!(e.size % e.block_size, 0);
    });
    let index_parsed = index
        .iter()
        .map(|e| SBFIndexEntry {
            ident: String::from_utf8_lossy(&e.ident.iter().take_while(|&&b| b != 0).map(|b| b.to_owned()).collect::<Vec<_>>()).to_string(),
            z1: e.z1,
            z2: e.z2,
            start: e.start,
            size: e.size,
            block_size: e.block_size,
            z3: e.z3,
        })
        .collect::<Vec<_>>();
    let entry1 = index_parsed.first().unwrap();
    process_chunk(&content[entry1.start as usize..(entry1.start + entry1.size) as usize])?;
    // dbg!(index_parsed);
    Ok(())
}

fn process_chunk(chunk: &[u8]) -> Result<()> {
    let mut orig_data = vec![];
    let mut parsed_data = vec![];
    for chunk in array_transmute::<_, SBFChunkData>(chunk).iter().take(1) {
        assert_eq!(chunk.scale1, chunk.scale2);
        dbg!(chunk.scale1);
        for b in chunk.content {
            orig_data.push(b);
            parsed_data.push(process_byte(b) / ((chunk.scale1).pow(2) + 1) as i16);
        }
    }
    std::fs::write("my.tmp.dir/orig.raw", orig_data)?;
    std::fs::write("my.tmp.dir/parsed.raw", array_transmute(parsed_data.as_slice()))?;
    Ok(())
}

fn process_byte(b: u8) -> i16 {
    let b = b as f32 - 127.5;
    let b = b / 140.0;
    let b = b * i16::MAX as f32;
    b as i16
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

#[derive(Debug)]
struct SBFIndexEntry {
    ident: String,
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
    scale3: u8,
    scale4: u8,
    content: [u8; 4096]
}