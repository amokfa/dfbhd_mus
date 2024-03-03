use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::str::FromStr;
use anyhow::{Result, Context};
use itertools::Itertools;
use libc::{MAP_PRIVATE, PROT_READ};
use dfbhd_mus::*;
use rayon::prelude::*;
use dfbhd_mus::cmd::cmd;

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

fn process_file(file: &Path, output: &Path) -> Result<()> {
    let content = unsafe {
        let file = File::open(file).context(format!("couldn't open {file:?}"))?;
        let size = file.metadata()?.len();
        let ptr = libc::mmap(null_mut(), size as _, PROT_READ, MAP_PRIVATE, file.as_raw_fd(), 0);
        std::slice::from_raw_parts(ptr as *const u8, size as _)
    };
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
        let wav_path = output.join("wav").join(format!("{prefix}.wav"));
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&wav_path)
            .unwrap();
        let mut total_size = 0;
        for e in es.iter() {
            let segment_blob = &content[e.start as usize..(e.start + e.size) as usize];
            for chunk in array_transmute::<_, SBFChunkData>(segment_blob) {
                total_size += chunk.size * 2;
            }
        }
        write_wav_header(&mut f, total_size).unwrap();
        let mut parsed_data = arrayvec::ArrayVec::<_, 8192>::new();
        for e in es.iter() {
            let segment_blob = &content[e.start as usize..(e.start + e.size) as usize];
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
    unsafe {
        libc::munmap(content.as_ptr() as _, content.len());
    }
    Ok(())
}

fn write_wav_header(writer: &mut File, total_size: u32) -> Result<()> {
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let sample_rate: u32 = 22050;
    let block_align = num_channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(block_align);

    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + total_size).to_le_bytes())?;
    writer.write_all(b"WAVE")?;

    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&num_channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;

    writer.write_all(b"data")?;
    writer.write_all(&total_size.to_le_bytes())?;

    Ok(())
}

fn upscale_pcm(b: u8, scale: u8) -> i16 {
    let b = b as i16 - 128;
    let b = b * 256;
    let b = b / 2i16.pow(scale as u32) / 2;
    b
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

// ....................................

#[repr(C)]
#[derive(Debug)]
struct SBFIndexEntryBin {
    ident: [u8; 8],
    z1: u32,
    z2: u32,
    start: u32,
    size: u32,
    block_size: u32, // 4104 = 4096 + 8
    z3: u32,
}

#[allow(unused)]
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