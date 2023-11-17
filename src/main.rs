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
use std::thread;
use std::time;

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

struct Display {
    pixels: Vec<Color>,
    width: u8,
    height: u8,
}

impl Display {
    fn from(canvas: &Canvas<sdl2::video::Window>) -> Display {
        let (width, height) = canvas.window().drawable_size();
        println!("Canvas: {} x {}", width, height);

        let size = width as usize * height as usize;
        Display {
            pixels: vec![Color::RGB(0, 0, 0); size],
            width: width as u8,
            height: height as u8,
        }
    }

    fn get_pixel(&self, x: u8, y: u8) -> Color {
        let addr = x as usize + y as usize * self.width as usize;
        return self.pixels[addr];
    }

    fn set_pixel(&mut self, x: u8, y: u8, color: Color) {
        let addr = x as usize + y as usize * self.width as usize;
        let max_addr = self.width as usize * self.height as usize;

        if addr < max_addr {
            self.pixels[addr] = color;
        }
    }

    fn render(&mut self, canvas: &mut Canvas<sdl2::video::Window>) {
        for y in 0..self.height {
            for x in 0..self.width {
                canvas.set_draw_color(self.get_pixel(x, y));
                let rect = Rect::new(x as i32, y as i32, 1, 1);
                canvas.fill_rect(rect).unwrap();
            }
        }
    }
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
}

fn get_tile_addr(x: u8, scx: u8, ly: u8, scy: u8) -> u16 {
    let ly = ly as u16;
    let x = x as u16;
    let scy = scy as u16;
    let scx = scx as u16;

    let intratile = (scy + ly) & 0b111;
    let tile_x = (x + scx) % 32;
    let tile_y = ((scy + ly) >> 3) % 32;
    println!("Intra: {}", intratile);
    println!("TX: {}, TY: {}", tile_x, tile_y);

    let tile_no = (tile_x + tile_y * 32) * 8 + intratile;
    return tile_no * 2;
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
    fn render(&mut self, rt: &mut runtime::Runtime, display: &mut Display) {
        let tile_addr = get_tile_addr(self.x, self.scx, self.ly, self.scy);

        let fst = rt.get(self.bg_offset() + tile_addr);
        let snd = rt.get(self.bg_offset() + tile_addr + 1);

        for i in 0..8 {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);
            let color = (h << 1) + l;

            let x = self.x * 8 + i;
            let y = self.ly;

            display.set_pixel(x, y, self.get_color(color));
        }

        self.x += 1;
        if self.x == 20 {
            self.x = 0; // hblank
            self.ly += 1;
            if self.ly > 153 {
                self.ly = 0;
            }
        }
        // rt.set()

        let r_status = set_bit(self.r_status, 2, self.ly == self.lyc);
        if get_bit(r_status, 6) == 1 && self.ly == self.lyc {
            // interrupt
        }


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

fn main() {
    let game_rom = load_rom("Tetris.gb");
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
    let mut ft = time::Instant::now();

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

        let cc = rt.tick();
        for _ in 0..cc {
            ppu.update(&rt);
            ppu.render(&mut rt, &mut display);
        }

        // Refresh 60fps
        if ft.elapsed() > refresh_target {
            canvas.clear();
            ft = time::Instant::now();
            display.render(&mut canvas);
            canvas.present();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_tile_addr() {
        let got = get_tile_addr(0, 0, 0, 0);
        assert_eq!(got, 0);
    }

    #[test]
    fn test_moves_in_x() {
        let got = get_tile_addr(1, 0, 0, 0);
        assert_eq!(got, 16);
    }

    #[test]
    fn test_moves_in_y() {
        let got = get_tile_addr(0, 0, 8, 0);
        assert_eq!(got, 32 * 8 * 2);
    }

    #[test]
    fn test_moves_in_tile_lines() {
        let got = get_tile_addr(0, 0, 1, 0);
        assert_eq!(got, 2);
    }

    #[test]
    fn test_moves_in_tile_lines_with_scy() {
        let got = get_tile_addr(0, 0, 0, 1);
        assert_eq!(got, 2);
    }

    #[test]
    fn test_moves_in_tile_lines_with_scy_intraline() {
        let got = get_tile_addr(0, 0, 8, 1);
        assert_eq!(got, 32 * 8 * 2 + 2);
    }

    #[test]
    fn test_moves_scx_tile_by_tile() {
        let got = get_tile_addr(1, 0, 8, 1);
        assert_eq!(got, 32 * 8 * 2 + 2 + 16);
    }
    #[test]
    fn test_selects_limit_right() {
        let got = get_tile_addr(31, 0, 32 * 4, 32 * 3 + 31);
        assert_eq!(got, 32 * 16 * 32 - 2);
    }

    #[test]
    fn test_moves_horizontally_with_scx() {
        let got = get_tile_addr(0, 1, 0, 0);
        assert_eq!(got, 16);
    }
}
