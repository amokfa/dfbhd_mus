use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use rodio::{OutputStream, Sink, Source};
use rodio::source::SeekError;
use dfbhd_mus::sbf::{upscale_pcm, write_wav_header, SBFChunkData, SBFIndexEntry, SBF};
use ncurses::*;
use dfbhd_mus::array_transmute;

fn main() {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let mut args = std::env::args();
    args.next();
    let sbf = SBF::from_file(PathBuf::from(args.next().unwrap()).as_path()).unwrap();
    dbg!(&sbf.chunks.keys());
    let track_name = args.next().unwrap();
    let output_dir = PathBuf::from(args.next().unwrap());

    let mut track = sbf.chunks.get(&track_name).unwrap().clone();
    #[derive(Default)]
    struct ReorderedData {
        chunk_pcms: Vec<Vec<i16>>,
        chunk_offsets: Vec<Duration>,
    }
    impl ReorderedData {
        fn recalc(&mut self, track: &[SBFIndexEntry], sbf: &SBF) {
            self.chunk_pcms.clear();
            self.chunk_offsets.clear();
            let mut curr_chunk_offset = Duration::ZERO;
            for chunk in track.iter() {
                self.chunk_offsets.push(curr_chunk_offset);
                let mut pcm_data = Vec::with_capacity(1024 * 1024);
                let segment_blob = &sbf.content[chunk.start as usize..(chunk.start + chunk.size) as usize];
                for chunk in array_transmute::<_, SBFChunkData>(segment_blob) {
                    for &b in &chunk.content[0..chunk.size as usize] {
                        let sample = upscale_pcm(b, chunk.scale1);
                        pcm_data.push(sample);
                    }
                }
                let chunk_duration_sex = 1.0 / 22050.0 * (pcm_data.len() / 2) as f64;
                let chunk_duration = Duration::from_secs_f64(chunk_duration_sex);
                curr_chunk_offset += chunk_duration;
                self.chunk_pcms.push(pcm_data);
            }
        }
    }
    let mut rd = ReorderedData::default();
    rd.recalc(&track, &sbf);
    let mut rd_dirty = false;
    let mut selected_chunk = 0;
    let mut playing_chunk = usize::MAX;
    nc_init();
    loop {
        if !sink.is_paused() {
            let passed = sink.get_pos();
            playing_chunk = 0;
            while playing_chunk < rd.chunk_offsets.len()-1 && rd.chunk_offsets[playing_chunk+1] < passed {
                playing_chunk += 1;
            }
        }

        clear();
        for idx in 0..track.len() {
            let attr_idx = if idx == selected_chunk && idx == playing_chunk {
                4
            } else if idx == selected_chunk {
                1
            } else if idx == playing_chunk {
                2
            } else {
                3
            };
            attron(COLOR_PAIR(attr_idx));
            let chunk = track.get(idx).unwrap();
            mvprintw(idx as _, 0, &format!("{}{}", chunk.ident, chunk.suffix)).unwrap();
            attroff(COLOR_PAIR(attr_idx));
        }
        refresh();

        let ch = getch();
        if ch == KEY_UP && selected_chunk != 0 {
            selected_chunk -= 1;
        } else if ch == KEY_DOWN && selected_chunk != track.len()-1 {
            selected_chunk += 1;
        } else if ch == KEY_LEFT && selected_chunk != track.len()-1 {
            let _ = sink.try_seek(sink.get_pos().checked_sub(Duration::from_secs(5)).unwrap_or(Duration::ZERO));
        } else if ch == KEY_RIGHT && selected_chunk != track.len()-1 {
            let _ = sink.try_seek(sink.get_pos() + Duration::from_secs(5));
        } else if ch == 'w' as _ && selected_chunk != 0 {
            let t = track.remove(selected_chunk);
            track.insert(selected_chunk-1, t);
            selected_chunk -= 1;
            rd_dirty = true;
        } else if ch == 's' as _ && selected_chunk != track.len()-1 {
            let t = track.remove(selected_chunk);
            track.insert(selected_chunk+1, t);
            selected_chunk += 1;
            rd_dirty = true;
        } else if ch == 'q' as _ {
            if rd_dirty {
                rd.recalc(&track, &sbf);
                rd_dirty = false;
            }
            let mut pcm_data = Vec::<i16>::with_capacity(1024 * 1024);
            for chunk_pcm in &rd.chunk_pcms {
                pcm_data.extend(chunk_pcm);
            }
            let source = RawPcmSource {
                data: Arc::new(pcm_data),
                channels: 2,
                sample_rate: 22050,
                index: 0,
            };

            sink.clear();
            sink.append(source);
            sink.try_seek(
                rd.chunk_offsets[selected_chunk]
                    .checked_sub(Duration::from_secs(2))
                    .unwrap_or(Duration::ZERO)
            ).unwrap();
            sink.play();
        } else if ch == 'a' as _ {
            sink.stop();
            sink.clear();
            playing_chunk = usize::MAX;
        } else if ch == 'e' as _ {
            if rd_dirty {
                rd.recalc(&track, &sbf);
                rd_dirty = false;
            }

            let mut samples = Vec::<i16>::with_capacity(1024 * 1024);
            for chunk_pcm in &rd.chunk_pcms {
                samples.extend(chunk_pcm);
            }
            let pcm_data = array_transmute::<_, u8>(&samples);
            let wav_path = output_dir.join(format!("{track_name}.wav"));
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&wav_path)
                .unwrap();
            write_wav_header(&mut f, pcm_data.len() as _).unwrap();
            f.write_all(pcm_data).unwrap();
            log(format!("exported to {:?}", wav_path));
        }
    }
}

fn nc_init() {
    std::panic::set_hook(Box::new(|info| {
        log(format!("Panic occurred: {}", info));
    }));

    initscr();               // Start curses mode
    cbreak();                // Disable line buffering
    noecho();                // Don't echo pressed keys to the screen
    keypad(stdscr(), true);  // Enable function keys and arrow keys
    timeout(100);
    start_color();
    init_pair(1, COLOR_RED, COLOR_BLACK);
    init_pair(2, COLOR_GREEN, COLOR_BLACK);
    init_pair(3, COLOR_WHITE, COLOR_BLACK);
    init_pair(4, COLOR_BLUE, COLOR_BLACK);
    let mut max_y = 0;
    let mut max_x = 0;
    getmaxyx(stdscr(), &mut max_y, &mut max_x);
}

fn log(s: impl Into<String>) {
    std::fs::write("/dev/pts/1", format!("{}\n", s.into())).unwrap()
}

struct RawPcmSource {
    data: Arc<Vec<i16>>, // Store PCM samples as signed 16-bit integers
    channels: u16,
    sample_rate: u32,
    index: usize,
}

impl Iterator for RawPcmSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.len() {
            let sample = self.data[self.index];
            self.index += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl Source for RawPcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.data.len() - self.index)
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let total_samples = self.data.len() as u32 / self.channels as u32;
        Some(Duration::from_secs_f64(total_samples as f64 / self.sample_rate as f64))
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        let total_samples = self.data.len() as u32 / self.channels as u32;
        let target_sample = (pos.as_secs_f32() * self.sample_rate as f32) as u32;

        // Ensure the target sample position is within bounds
        if target_sample >= total_samples {
            self.index = self.data.len();
            return Ok(());
        }

        // Update the index to the target position in samples
        self.index = (target_sample * self.channels as u32) as usize;

        Ok(())
    }
}
