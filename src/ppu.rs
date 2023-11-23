use crate::byteop::*;
use crate::memory::Memory;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::rect::Rect;

pub struct PPU {
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
    wait: u16,
}

impl PPU {
    pub fn new() -> PPU {
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
            wait: 0,
        }
    }
    fn get_color(&self, id: u8) -> Color {
        let colors = vec![
            // Color::RGB(155, 188, 15),
            // Color::RGB(139, 172, 15),
            // Color::RGB(48, 98, 48), // dark green
            // Color::RGB(15, 56, 15), // darkest green
            Color::RGB(255, 255, 255),
            Color::RGB(169, 169, 169),
            Color::RGB(84, 84, 84),
            Color::RGB(0, 0, 0),
        ];
        return colors[id as usize];
    }

    pub fn update(&mut self, rt: &mut impl Memory, cc: u8, display: &mut Display) {
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

        let cc = cc as u16 * 4;
        if self.wait <= cc {
            self.wait = 0;
        } else if self.wait > 0 {
            self.wait -= cc;
        }

        if self.wait == 0 {
            self.render(rt, display);
        }
    }

    // render background
    fn render(&mut self, rt: &mut impl Memory, display: &mut Display) {
        let tile_addr = get_tile_addr(self.x, self.scx, self.ly, self.scy);
        let bg_tilemap = get_bit(self.r_control, 3);

        let tile_id = rt.get(self.tile_offset(bg_tilemap) + tile_addr);
        let ttr = self.get_tile(tile_id, (self.ly as u16 + self.scy as u16) as u8);
        self.render_tile(display, rt, ttr, self.x, self.ly);

        let window_enable = get_bit(self.r_control, 5) == 1;
        let window_visible = (0..=166).contains(&self.wx) && (0..=143).contains(&self.wy);
        let window_tilemap = get_bit(self.r_control, 6);

        if window_enable && window_visible && self.ly == self.wy {
            let tile_addr = self.x as u16 + self.ly as u16 * 32;
            let tile_id = rt.get(self.tile_offset(window_tilemap) + tile_addr);

            let ttr = self.get_tile(tile_id, self.ly);
            self.render_tile(display, rt, ttr, self.wx + self.x, self.wy);
        }

        self.x += 1;
        if self.x == 20 {
            self.x = 0; // hblank
            self.wait = 456;
            self.ly += 1;

            let vblank = self.ly >= 144;
            let interrupt_flag = rt.get(0xFF0F);
            rt.set(0xFF0F, set_bit(interrupt_flag, 0, vblank));

            if self.ly > 153 {
                self.ly = 0;
            }
        }

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

    fn tile_offset(&self, id: u8) -> u16 {
        return if id == 0 { 0x9800 } else { 0x9C00 };
    }

    fn get_tile(&self, tile: u8, y: u8) -> u16 {
        let intratile = (y & 0b111) as u16;

        if get_bit(self.r_control, 4) == 0 {
            0x8800u16.wrapping_add((tile as i8) as u16) + intratile * 2
        } else {
            0x8000 + (tile as u16 * 16) + intratile * 2
        }
    }

    fn render_tile(&self, display: &mut Display, rt: &impl Memory, ttr: u16, x: u8, y: u8) {
        let fst = rt.get(ttr + 1);
        let snd = rt.get(ttr + 0);

        // let fst = rt.get(self.bg_offset() + tile_addr);
        // let snd = rt.get(self.bg_offset() + tile_addr + 1);

        for i in 0..8 {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);
            let color = (h << 1) + l;

            let x = x * 8 + (7 - i);
            let y = y;

            display.set_pixel(x, y, self.get_color(color));
        }
    }
}

fn get_tile_addr(x: u8, scx: u8, ly: u8, scy: u8) -> u16 {
    let ly = ly as u16;
    let x = x as u16;
    let scy = scy as u16;
    let scx = scx as u16;

    let tile_x = (x + (scx >> 3)) % 32;
    let tile_y = ((scy + ly) >> 3) % 32;

    let tile_no = tile_x + (tile_y * 32);

    return tile_no & 0x3FF;
}

pub struct Display {
    pixels: Vec<Color>,
    width: u8,
    height: u8,
}

impl Display {
    pub fn from(canvas: &Canvas<sdl2::video::Window>) -> Display {
        let (width, height) = canvas.window().drawable_size();

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

    pub fn render(&mut self, canvas: &mut Canvas<sdl2::video::Window>) {
        for y in 0..self.height {
            for x in 0..self.width {
                canvas.set_draw_color(self.get_pixel(x, y));
                let rect = Rect::new(x as i32, y as i32, 1, 1);
                canvas.fill_rect(rect).unwrap();
            }
        }
    }
}

