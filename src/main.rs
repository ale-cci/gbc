// https://retrocomputing.stackexchange.com/questions/11732/how-does-the-gameboys-memory-bank-switching-work
// https://realboyemulator.wordpress.com/2013/01/03/a-look-at-the-game-boy-bootstrap-let-the-fun-begin/
// https://gekkio.fi/files/gb-docs/gbctr.pdf
extern crate sdl2;
use std::fs;
use std::io::Read;
mod runtime;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use std::time::Duration;
mod byteop;
use crate::byteop::*;

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

#[derive(Debug)]
struct Sprite {
    x: u8,
    y: u8,
    tile: u8,
    _flags: u8,
}

impl Sprite {
    fn at(rt: &runtime::Runtime, addr: u16) -> Sprite {
        let y_pos = rt.get(addr);
        let x_pos = rt.get(addr + 1);
        let tile = rt.get(addr + 2);
        let flags = rt.get(addr + 3);

        Sprite {
            y: y_pos,
            x: x_pos,
            tile,
            _flags: flags,
        }
    }

    fn priority(&self) -> u8 {
        return get_bit(self._flags, 7);
    }
    fn y_flip(&self) -> u8 {
        return get_bit(self._flags, 6);
    }
    fn x_flip(&self) -> u8 {
        return get_bit(self._flags, 5);
    }
    fn palette(&self) -> u8 {
        return get_bit(self._flags, 4);
    }
}

struct PPU {
    x: u8,

    r_control: u8,
    r_status: u8,
    scx: u8,
    scy: u8,
    ly: u8,
    lyc: u8,
    obp0: u8,
    obp1: u8,
    wx: u8,
    wy: u8,

    window_line_counter: u8,
    remaining_cycles: u8,
}

impl PPU {
    fn new() -> PPU {
        PPU {
            x: 0,
            r_control: 0,
            r_status: 0,
            scx: 0,
            scy: 0,
            ly: 0,
            lyc: 0,
            obp0: 0,
            obp1: 0,
            wx: 0,
            wy: 0,
            window_line_counter: 0,
            remaining_cycles: 0,
        }
    }
    fn get_color(&self, id: u8) -> Color {
        let colors = vec![
            Color::RGB(155, 188, 15),
            Color::RGB(139, 172, 15),
            Color::RGB(48, 98, 48), // dark green
            Color::RGB(15, 56, 15), // darkest green
        ];
        return colors[id as usize];
    }

    fn update(&mut self, rt: &runtime::Runtime) {
        self.r_control = rt.get(0xFF40);
        self.r_status = rt.get(0xFF41);
        self.scy = rt.get(0xFF42);
        self.scx = rt.get(0xFF43);
        self.ly = rt.get(0xFF44);
        self.lyc = rt.get(0xFF45); // 0..=153
        self.obp0 = rt.get(0xFF48);
        self.obp1 = rt.get(0xFF49);
        self.wy = rt.get(0xFF4A);
        self.wx = rt.get(0xFF4B);
    }

    // render background
    fn render(&mut self, e_cpu: u8, rt: &mut runtime::Runtime, display: &mut Display) {
        if self.remaining_cycles <= e_cpu {
            self.remaining_cycles = 0;
        } else {
            self.remaining_cycles -= e_cpu;
        }
        if self.remaining_cycles != 0 {
            return;
        }

        let coord_y = (self.ly as u16 + self.scy as u16) & 0xff;

        let tile_voff = (coord_y & 0b111) * 32 * 2;
        let tile_line = coord_y >> 3;

        let tile_no = (self.x as u16 + tile_voff) + tile_line;

        let tile_addr = (2 * tile_no) & 0x3ff;
        let fst = rt.get(self.bg_offset() + tile_addr);
        let snd = rt.get(self.bg_offset() + tile_addr + 1);

        for i in 0..8 {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);
            let color = (h << 1) + l;

            display.set_pixel(self.x * 8+ i, self.ly, self.get_color(color));
        }

        self.x += 1;
        if self.x == 20 {
            self.x = 0; // hblank
            self.ly += 1;

            let vblank = self.ly >= 144;
            let interrupt_flag = rt.get(0xFF0F);
            rt.set(0xFF0F, set_bit(interrupt_flag, 0, vblank));

            if self.ly > 153 {
                self.ly = 0;
            }
        }
        // rt.set()

        let mut r_status = self.r_status;
        if self.ly == self.lyc {
            if get_bit(self.r_status, 6) == 1 {
                // LYC int select Trigger interrupt if
                let ie = rt.get(0xFFFF);
                rt.set(0xFFFF, set_bit(ie, 0, true));
            }
            r_status = set_bit(r_status, 2, true);
        }

        // r_status 1-0: ppu mode
        rt.set(0xFF41, r_status);
        rt.set(0xFF44, self.ly);
    }

    fn bg_offset(&self) -> u16 {
        return if get_bit(self.r_control, 3) == 0 {
            0x9800
        } else {
            0x9C00
        };
    }
}
struct Display {
    pixels: Vec<Color>,
    width: u8,
    height: u8,
    scale: u8,
}
impl Display {
    fn new() -> Display {
        Display {
            pixels: vec![Color::RGB(255, 255, 255); 160 * 144],
            width: 160,
            height: 144,
            scale: 1,
        }
    }
    fn set_pixel(&mut self, x: u8, y: u8, c: Color) {
        let addr = y as usize * self.width as usize + x as usize;
        if addr < self.width as usize * self.height as usize {
            self.pixels[addr] = c;
        }
    }
    fn get_pixel(&self, x: u8, y: u8) -> Color {
        let addr = y as usize * self.width as usize + x as usize;
        return self.pixels[addr];
    }
    fn render(&self, canvas: &mut Canvas<sdl2::video::Window>) {
        for y in 0..self.height {
            for x in 0..self.width {
                let color = self.get_pixel(x, y);
                canvas.set_draw_color(color);
                canvas
                    .fill_rect(Rect::new(
                        x as i32 * self.scale as i32,
                        y as i32 * self.scale as i32,
                        (self.scale).into(),
                        (self.scale).into(),
                    ))
                    .unwrap();
            }
        }
    }
}

fn main() {
    let game_rom = load_rom("Tetris.gb");
    let bootstrap = load_rom("DMG_ROM.bin");

    let mut rt = runtime::Runtime::load(&bootstrap, &game_rom);

    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();
    let window = video
        .window("gbc", 160, 144)
        .position_centered()
        .build()
        .unwrap();
    let mut display = Display::new();
    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(255, 255, 255));
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut ppu = PPU::new();
    'running: loop {
        canvas.clear();
        let cc = rt.tick();

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
        ppu.update(&rt);
        ppu.render(cc, &mut rt, &mut display);

        display.render(&mut canvas);
        canvas.present();
        let tick = 1_000_000_000u32 / 419_000_000;
        ::std::thread::sleep(Duration::new(0, tick));
    }
}
