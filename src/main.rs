extern crate sdl2;
use std::fs;
use std::io::Read;
mod runtime;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
mod byteop;
use clap::Parser;
use std::time;
mod ppu;
mod timer;
use crate::ppu::{Display, PPU};
use sdl2::pixels::Color;
mod memory;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg()]
    rom: String,
}

fn load_rom(filename: &str) -> Vec<u8> {
    let mut f = fs::File::open(filename).expect(&format!(
        "File `{}´ not found in current working directory",
        filename
    ));
    let meta = f
        .metadata()
        .expect(&format!("Unable to read `{}´ metadata", filename));

    let mut rom = vec![0; meta.len() as usize];
    f.read(&mut rom).expect("Overflow");
    return rom;
}

fn main() {
    let args = Args::parse();

    let game_rom = load_rom(&args.rom);
    let bootstrap = load_rom("DMG_ROM.bin");

    let mut rt = runtime::Runtime::load(&bootstrap, &game_rom);

    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();
    let width = 160;
    let height = 144;

    let window = video
        .window("gbc", width, height)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let mut display = Display::from(&canvas);

    canvas.set_draw_color(Color::RGB(255, 255, 255));
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut ppu = PPU::new();

    let refresh_target = time::Duration::from_micros(10000000 / 60);
    let clock_target = time::Duration::from_nanos(1_000_000_000 / 4194304);

    let mut ft = time::Instant::now();
    let mut tick = time::Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        if tick.elapsed() > clock_target {
            let cc = rt.tick();
            // rt.tick_timer(cc);
            ppu.update(&mut rt, cc, &mut display);
        }

        tick = time::Instant::now();

        // Refresh 60fps
        if ft.elapsed() > refresh_target {
            // println!("Tick: {:?} ~0.25µs ({:?})", tick.elapsed(), clock_target);
            canvas.clear();
            ft = time::Instant::now();
            display.render(&mut canvas);
            canvas.present();
        }
    }
}
