use anyhow::Context;
use itertools::Itertools;
use libc::{MAP_PRIVATE, PROT_READ};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::null_mut;

use crate::array_transmute;

pub struct SBF {
    pub content: &'static [u8],
    pub header: SBFHeader,
    pub chunks: Vec<SBFIndexEntry>,
    pub grouped_chunks: HashMap<String, Vec<SBFIndexEntry>>,
}

impl SBF {
    pub fn from_file(file: &Path) -> anyhow::Result<Self> {
        let content = unsafe {
            let file = File::open(file).context(format!("couldn't open {file:?}"))?;
            let size = file.metadata()?.len();
            let ptr = libc::mmap(
                null_mut(),
                size as _,
                PROT_READ,
                MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            );
            std::slice::from_raw_parts(ptr as *const u8, size as _)
        };
        let header =
            array_transmute::<_, SBFHeader>(&content[0..size_of::<SBFHeader>()])[0].clone();
        let index: &[SBFIndexEntryBin] = array_transmute(
            &content[header.index_offset as usize
                ..(header.index_offset as usize
                    + header.index_count as usize * size_of::<SBFIndexEntryBin>())],
        );
        index.iter().for_each(|e| {
            // assert_eq!(e.z1, 0); // non zero for only one chunk (VALIANT0)
            assert_eq!(e.z2, 0);
            assert_eq!(e.z3, 0);
            assert_eq!(e.block_size, 4104);
            assert_eq!(e.size % e.block_size, 0);
        });
        let chunks = index
            .iter()
            .map(|index| {
                let (ident, suffix) = if index.ident[0] == 'm' as u8 {
                    let split_point = index.ident.iter().take_while(|&&b| b != 'a' as u8).count();
                    (
                        index
                            .ident
                            .iter()
                            .take(split_point)
                            .map(|b| b.to_owned())
                            .collect::<Vec<_>>(),
                        index
                            .ident
                            .iter()
                            .skip(split_point)
                            .take_while(|&&b| b != 0)
                            .map(|b| b.to_owned())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    let split_point = index
                        .ident
                        .iter()
                        .take_while(|b| b.is_ascii_alphabetic())
                        .count();
                    (
                        index
                            .ident
                            .iter()
                            .take(split_point)
                            .map(|b| b.to_owned())
                            .collect::<Vec<_>>(),
                        index
                            .ident
                            .iter()
                            .skip(split_point)
                            .take_while(|&&b| b != 0)
                            .map(|b| b.to_owned())
                            .collect::<Vec<_>>(),
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

        let grouped_chunks = chunks
            .iter().cloned()
            .group_by(|index| index.ident.to_owned())
            .into_iter()
            .map(|(key, group)| (key, group.collect::<Vec<_>>()))
            .collect::<HashMap<_, _>>();

        Ok(Self {
            content,
            header,
            chunks,
            grouped_chunks,
        })
    }
}

impl Drop for SBF {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.content.as_ptr() as _, self.content.len());
        }
    }
}

pub fn process_file(file: &Path, output: &Path) -> anyhow::Result<()> {
    let sbf = SBF::from_file(file)?;
    sbf.grouped_chunks.par_iter().for_each(|(prefix, es)| {
        let wav_path = output.join("wav").join(format!("{prefix}.wav"));
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&wav_path)
            .unwrap();
        let mut total_size = 0;
        for e in es.iter() {
            let segment_blob = &sbf.content[e.start as usize..(e.start + e.size) as usize];
            for chunk in array_transmute::<_, SBFChunkData>(segment_blob) {
                total_size += chunk.size * 2;
            }
        }
        write_wav_header(&mut f, total_size).unwrap();
        let mut parsed_data = arrayvec::ArrayVec::<_, 8192>::new();
        for e in es.iter() {
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
    Ok(())
}

pub fn write_wav_header(writer: &mut File, total_size: u32) -> anyhow::Result<()> {
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

pub fn upscale_pcm(b: u8, scale: u8) -> i16 {
    let b = b as i16 - 128;
    let b = b * 256;
    let b = b / 2i16.pow(scale as u32) / 2;
    b
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct SBFHeader {
    magic: [u8; 4],
    i1: u32,
    i2: u32,
    i3: u32,
    index_offset: u32,
    index_count: u32,
}

// ....................................

#[repr(C)]
#[derive(Debug)]
pub struct SBFIndexEntryBin {
    pub ident: [u8; 8],
    pub z1: u32,
    pub z2: u32,
    pub start: u32,
    pub size: u32,
    pub block_size: u32, // 4104 = 4096 + 8
    pub z3: u32,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct SBFIndexEntry {
    pub ident: String,
    pub suffix: String,
    pub z1: u32,
    pub z2: u32,
    pub start: u32,
    pub size: u32,
    pub block_size: u32,
    pub z3: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct SBFChunkData {
    pub size: u32,
    pub scale1: u8,
    pub scale2: u8,
    pub two_fifty: u8,
    pub zero: u8,
    pub content: [u8; 4096],
}
