extern crate sdl2;

mod registers;
use std::fs;
use std::io::Read;
use std::rc::Rc;
mod runtime;
use mbc::{Rom, RomMBC3, RomNoMBC};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
mod byteop;
use clap::Parser;
use std::time;
mod ppu;
mod timer;
use crate::apu::APU;
use crate::ppu::{Display, PPU};
use sdl2::pixels::Color;
mod memory;
use memory::HWInput;
mod apu;
mod mbc;

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

fn get_btn(sdl_key: &str) -> Option<HWInput> {
    return match sdl_key {
        "I" => Some(HWInput::ArrUp),
        "K" => Some(HWInput::ArrDown),
        "J" => Some(HWInput::ArrLeft),
        "L" => Some(HWInput::ArrRight),
        "A" => Some(HWInput::BtnA),
        "B" => Some(HWInput::BtnB),
        "Return" => Some(HWInput::BtnStart),
        "Space" => Some(HWInput::BtnSelect),
        _ => None,
    };
}

enum RomMBC<'a> {
    RomMBC3(RomMBC3<'a>),
    RomNoMBC(RomNoMBC<'a>),
}

impl Rom<'_> for RomMBC<'_> {
    fn set(&mut self, addr: u16, val: u8) {
        return match self {
            RomMBC::RomMBC3(item) => item.set(addr, val),
            RomMBC::RomNoMBC(item) => item.set(addr, val),
        }
    }
    fn get(&self, addr: u16) -> u8 {
        return match self {
            RomMBC::RomMBC3(item) => item.get(addr),
            RomMBC::RomNoMBC(item) => item.get(addr),
        }
    }
}

fn main() {
    let args = Args::parse();

    let game_rom = load_rom(&args.rom);
    let bootstrap = load_rom("DMG_ROM.bin");

    let mbc_type = game_rom[0x0147];

    let mut rom: RomMBC = match mbc_type {
        0x13 => RomMBC::RomMBC3(RomMBC3::new(&game_rom)),
        0..=3 => RomMBC::RomNoMBC(RomNoMBC{ rom: &game_rom }),
        code => panic!("Unsupported mbc type {:#x}", code),
    };

    let mut rt = runtime::Runtime::load(&bootstrap, &mut rom);

    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();
    let audio = sdl_context.audio().unwrap();
    let width = 160;
    let height = 144;

    let mut apu = APU::new();

    let device = audio
        .open_playback(None, &apu.spec.clone(), |_sample| &mut apu)
        .unwrap();

    device.resume();

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

    let refresh_target = time::Duration::from_micros(1_000_000 / 60);
    let clock_target = time::Duration::from_nanos(1_000_000_000 / 4194304).as_nanos();

    let mut ft = time::Instant::now();
    let mut tick = time::Instant::now();

    let speed = 1;
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,

                Event::KeyDown {
                    keycode: Some(key), ..
                } => {
                    if let Some(btn) = get_btn(&key.name()) {
                        rt.press_btn(btn);
                    }
                }

                Event::KeyUp {
                    keycode: Some(key), ..
                } => {
                    if let Some(btn) = get_btn(&key.name()) {
                        rt.release_btn(btn);
                    }
                }

                _ => {}
            }
        }

        let ticks = (tick.elapsed().as_nanos() / clock_target) / (4 * speed);
        for _ in 0..ticks {
            let cc = rt.tick();
            rt.tick_timer(cc * 4);
            ppu.update(&mut rt, cc, &mut display);
            apu.update(cc * 4, &mut rt);
        }
        if ticks > 0 {
            tick = time::Instant::now();
        }

        // Refresh 60fps
        if ft.elapsed() > refresh_target {
            // println!("Tick: {:?} ~0.25µs ({:?})", tick.elapsed(), clock_target);
            canvas.clear();
            ft = time::Instant::now();
            display.render(&mut canvas);
            canvas.present();
        }
    }
    drop(rt);
}
