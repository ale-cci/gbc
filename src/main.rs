// https://retrocomputing.stackexchange.com/questions/11732/how-does-the-gameboys-memory-bank-switching-work
// https://realboyemulator.wordpress.com/2013/01/03/a-look-at-the-game-boy-bootstrap-let-the-fun-begin/
// https://gekkio.fi/files/gb-docs/gbctr.pdf
extern crate sdl2;
use std::fs;
use std::io::Read;
mod runtime;
use std::time::Duration;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::rect::Rect;

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

fn get_bit(reg: u8, pos: u8) -> u8 {
    return (reg & (1 << pos)) >> pos;
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
}

impl PPU {
    fn update(&mut self,  rt: &runtime::Runtime) {
        self.r_control = rt.get(0xFF40);
        self.r_status = rt.get(0xFF41);
        self.scx = rt.get(0xFF42);
        self.scy = rt.get(0xFF43);
        self.ly = rt.get(0xFF44);
        self.lyc = rt.get(0xFF45);
        self.obp0 = rt.get(0xFF48);
        self.obp1 = rt.get(0xFF49);
        self.wx = rt.get(0xFF4A);
        self.wy = rt.get(0xFF4B);
    }
    fn fetch(&self, rt: &runtime::Runtime) {
        let mut tile_no = (self.x + ((self.scx / 8) & 0x1f)) as u16;
        tile_no += 32u16 * (((self.ly as u16 + self.scy as u16) & 0xff) / 8); // only if fetching
                                                                              // bg pixels
        tile_no &= 0x3ff;

        let b0 = rt.get(self.bg_offset() + tile_no);
        let b1 = rt.get(self.bg_offset() + tile_no + 1);
    }

    fn bg_offset(&self) -> u16 {
        return if get_bit(self.r_control, 3) == 1 {
            0x9C00
        } else {
            0x9800
        };
    }

}

enum PPUAddrMode {
    M8000,
    M8800,
}
// PPU
fn render_screen(rt: &runtime::Runtime, canvas: &mut Canvas<sdl2::video::Window>) {
    // 00 01 10 11 (from darker to lighter)
    // 8x8 pix
    let colors = vec![
        Color::RGB(155, 188, 15),
        Color::RGB(139, 172, 15),
        Color::RGB(48, 98, 48), // dark green
        Color::RGB(15, 56, 15), // darkest green
    ];

    let r_control = rt.get(0xFF40);
    let r_status = rt.get(0xFF41);
    let scx = rt.get(0xFF42);
    let scy = rt.get(0xFF43);

    println!("Regs: r_control: {:#b} r_status: {:#b}", r_control, r_status);

    if get_bit(r_control, 7) == 0 { // LCD enable
        canvas.set_draw_color(Color::RGB(0xff, 0xff, 0xff));
        return
    }
    let bgw_enable = get_bit(r_control, 0) == 1;
    let sprite_enable = get_bit(r_control, 1) == 1;
    let high_sprites = get_bit(r_control, 2) == 1;

    let bg_select = get_bit(r_control, 3); // 1 9C00-9FFF else 9800-9BFFF
    let bg_offset = if bg_select == 1 { 0x9C00 } else { 0x9800 };

    let mode_8000 = get_bit(r_control, 4) == 1;
    let widow_display_enable = get_bit(r_control, 5) == 1;

    if bgw_enable {
        // draw background
        for y in 0..18 {
            for x in 0..20 {
                let first = rt.get(bg_offset + (y * 18 + x) * 2);
                let second = rt.get(bg_offset + (y * 18 + x) * 2 + 1);

                for px in 0..8 {
                    let h = get_bit(second, px);
                    let l = get_bit(first, px);

                    let color = (h << 1) + l;

                    canvas.set_draw_color(colors[color as usize]);
                    canvas.fill_rect(
                        Rect::new((x  as u16 * 8+ px as u16) as i32, y.into(), 1, 1));
                }
            }
        }
    }
    let ly = 0;
    for i in 0 .. (0xFE9F - 0xFE00) / 4 {
        let sprite = Sprite::at(rt, 0xFE9F + i * 4);
        if sprite.x > 0 && ly + 16 >= sprite.y {
            println!("S: {:?}", sprite);
            if mode_8000 {
                let pix = rt.get(0x8000u16 + (sprite.tile as u16) * 16);
                println!("pix: {:b}", pix);
            } else {
                let addr = 0x9000u16.wrapping_add(((sprite.tile as i8) as i16 * 16) as u16);
                let pix = rt.get(addr as u16);

                println!("pix: {:b}", pix);
            }

        }
    }

    canvas.set_draw_color(colors[0]);
    canvas.fill_rect(Rect::new(0, 0, 1, 1));
    canvas.set_draw_color(colors[3]);
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
    let mut canvas = window.into_canvas().build().unwrap();

    canvas.set_draw_color(Color::RGB(255, 255, 255));
    let mut event_pump = sdl_context.event_pump().unwrap();

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
        render_screen(&rt, &mut canvas);

        canvas.present();

        1_000_000_000u32 / 60; // fps
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
