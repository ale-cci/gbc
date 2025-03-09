use crate::byteop::*;
use crate::memory::Memory;
use crate::registers;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::collections::VecDeque;
use std::option::Option;
use sdl2::render::Canvas;

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
    bgp: u8,
    sprites: Vec<Sprite>,

    ppu_state: u8,

    wait: u16,
    waited: u16,
    filtered_sprites: Vec<Sprite>,

    pixel_fifo_bg: VecDeque<FIFOPixel>,

}

#[derive(Copy, Clone)]
enum FIFOPixelSource {
    BACKGROUND,
    WINDOW,
    SPRITE(Sprite),
}

#[derive(Copy, Clone)]
struct FIFOPixel {
    source: FIFOPixelSource,
    color_id: u8,
}

#[derive(Clone, Default, Copy)]
struct Sprite {
    addr: u16,
    x: u8,
    y: u8,
    tile: u8,
    flags: u8,
}

const PALETTE: [Color; 4] = [
    Color::RGB(255, 255, 255),
    Color::RGB(169, 169, 169),
    Color::RGB(84, 84, 84),
    Color::RGB(0, 0, 0),
];

fn color_from_code(code: usize) -> Color {
    return PALETTE[code];
}

impl Sprite {
    fn new(addr: u16) -> Sprite {
        let s = Sprite {
            addr: 0xFE00u16 + addr,
            x: 0,
            y: 0,
            tile: 0,
            flags: 0,
        };
        return s;
    }

    fn load(&mut self, rt: &impl Memory) {
        self.y = rt.get(self.addr + 0);
        self.x = rt.get(self.addr + 1);
        self.tile = rt.get(self.addr + 2);
        self.flags = rt.get(self.addr + 3);
    }

    fn is_visible(&self, ly: u8, lcdc_2: u8) -> bool {
        let height = if lcdc_2 == 0 { 8 } else { 16 };
        return self.x != 0 && (ly + 16 >= self.y) && (ly + 16 < self.y + height);
    }

    fn tile_line(&self, mem: &impl Memory, y: u8) -> (u8, u8) {
        let tile_id = tile_addr(self.tile, false);
        let shift_y = y >> 3;
        let intratile = y & 0b111;

        let addr = tile_id + intratile as u16 * 2;

        let fst = mem.get(addr);
        let snd = mem.get(addr + 1);

        return (fst, snd);
    }

    fn palette(&self) -> u8 {
        return get_bit(self.flags, 4);
    }
}

impl PPU {
    pub fn new() -> PPU {
        let mut sprites = Vec::with_capacity(40);
        for i in 0..40 {
            sprites.push(Sprite::new(i as u16 * 4));
        }
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
            wait: 0,
            bgp: 0,
            sprites,

            ppu_state: 1,
            waited: 0,

            filtered_sprites: Vec::with_capacity(10),

            pixel_fifo_bg: VecDeque::with_capacity(16),
        }
    }

    fn get_color(&self, id: u8, palette: u8) -> u8 {
        let shift = id * 2;
        let color = (palette & (0b11 << shift)) >> shift;
        return color;
    }

    pub fn update(&mut self, rt: &mut impl Memory, dots: u8, display: &mut Display) {
        self.r_control = rt.get(registers::LCDC);
        self.r_status = rt.get(registers::STAT);
        self.scy = rt.get(registers::SCY);
        self.scx = rt.get(registers::SCX);
        self.ly = rt.get(registers::LY);
        self.lyc = rt.get(registers::LYC); // 0..=153
        self.bgp = rt.get(registers::BGP);
        self.obp0 = rt.get(registers::OBP0);
        self.obp1 = rt.get(registers::OBP1);
        self.wy = rt.get(registers::WY);
        self.wx = rt.get(registers::WX);
        let mut dots: u16 = dots.into();

//        // mode 2. OAM scan, read values from RAM
//        if self.ppu_state == 2 {
//            for s in &mut self.sprites {
//                s.load(rt);
//            }
//        }

        while dots > 0 {
            if self.wait > 0 {
                if self.wait >= dots {
                    self.wait -= dots;
                    dots = 0;
                } else {
                    dots -= self.wait;
                    self.wait = 0;
                }
            } else {
                self.render(rt, display);
            }
        }
        self.update_registers(rt);

        assert!(rt.get(registers::IF) & 0b1 == (self.ly >= 144) as u8);
    }

    fn render(&mut self, rt: &mut impl Memory, display: &mut Display) {
        assert!(self.ly < 154);
        if self.ppu_state == 1 {
            if self.ly == 0 || self.ly == 153 {
                self.ly = 0;
                // enter mode 2, start waiting 80 dots.
                self.ppu_state = 2;
                self.wait = 80;
                assert!(self.waited == 0);
                self.waited += 80;
            } else if self.ly >= 144 {
                // VBLANK
                self.wait = 456;
                self.ly += 1;
            } else {
                panic!("Impossible, PPU on mode 1 when ly is {}", self.ly);
            }
            return;
        } else if self.ppu_state == 2 {
            // 80 dots just waited, starting mode 3.
            // enter mode 3
            if self.waited != 80 {
                panic!("Self waited is not 80: {}", self.waited);
            }
            assert!(self.waited == 80);
            self.wait = 12;
            self.waited += 12;
            self.ppu_state = 3;
            return;
        } else if self.ppu_state == 3 {
            if self.waited == 80 + 12 {
                for s in &mut self.sprites {
                    s.load(rt);
                }

                let obj_size = get_bit(self.r_control, 2);
                self.filtered_sprites.clear();

                for s in &self.sprites {
                    if self.filtered_sprites.len() == 10 {
                        break;
                    }

                    if s.is_visible(self.ly, obj_size) && s.x > 0 {
                        self.filtered_sprites.push(s.clone());
                    }
                }
                self.wait = 10 * self.filtered_sprites.len() as u16;
                self.waited += self.wait;
            }
            assert!(self.ly <= 144);

            // 12 dots waited, start drawing a pixel for dot.

            self.wait += 8;
            self.waited += 8;

            self.fetch_pixels(rt);
            self.draw_pixels(display);
            // if bg_window_enable_priority {
            //     self.render_bg(rt, display);
            //     self.render_objects(rt, display);
            // } else {
            //     self.render_bg(rt, display);
            //     self.render_objects(rt, display);
            // }

            self.x += 1;
            if self.x == 20 {
                // enter HBLANK
                assert!(self.waited >= 80 + 172 && self.waited <= 80 + 289);
                self.wait = 456 - self.waited;
                assert!(self.wait >= 87 && self.wait <= 204);
                self.waited = 0;
                self.ppu_state = 0;
            }
        } else if self.ppu_state == 0 {
            assert!(self.waited == 0);
            self.ly += 1;
            self.x = 0;
            if self.ly >= 144 {
                self.ppu_state = 1;
                self.wait = 456;
            } else {
                self.wait = 80;
                self.waited += 80;
                self.ppu_state = 2;
            }
        }
    }

    fn update_registers(&mut self, rt: &mut impl Memory) {
        let mut r_status = self.r_status;

        // setting the interrupt flags
        let mut reg_if = rt.get(registers::IF);

        reg_if = set_bit(reg_if, 0, self.ppu_state == 1);
        reg_if = set_bit(reg_if, 1, self.ly == self.lyc);

        // https://gbdev.io/pandocs/STAT.html#ff41--stat-lcd-status
        let mut stat_int = false;
        if get_bit(r_status, 3) == 1 && self.ppu_state == 0 {
            stat_int |= true;
        }

        if get_bit(r_status, 4) == 1 && self.ppu_state == 1 {
            stat_int |= true;
        }

        if get_bit(r_status, 5) == 1 && self.ppu_state == 2 {
            stat_int |= true;
        }

        if get_bit(r_status, 6) == 1 && self.ly == self.lyc {
            stat_int |= true;
        }

        r_status = set_bit(r_status, 2, self.ly == self.lyc);
        r_status &= 0b11111100;
        r_status |= self.ppu_state;

        reg_if = set_bit(reg_if, 1, stat_int);

        rt.set(registers::IF, reg_if);
        rt.set(registers::STAT, r_status); // stat register
        rt.set(registers::LY, self.ly); // current horizontal line
    }

    fn tile_offset(&self, id: u8) -> u16 {
        return if id == 0 { 0x9800 } else { 0x9C00 };
    }

    /// Calculates the memory address of the tile, given SCY & tile_numbere
    fn get_tile(&self, tile: u8, scy: u8) -> u16 {
        let mode_8800 = get_bit(self.r_control, 4) == 0;

        let intratile = (scy & 0b111) as u16;
        return tile_addr(tile, mode_8800) + (intratile * 2);
    }

    // ttr: tile to render
    fn render_tile_pixel(&self, display: &mut Display, rt: &impl Memory, ttr: u16, x: u8, y: u8, scx: u8) {
        let fst = rt.get(ttr + 0);
        let snd = rt.get(ttr + 1);

        for i in 0..8 {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);
            let color = (h << 1) + l;

            let x = x * 8 + (7 - i);
            let y = y;

            display.set_pixel(x + (scx & 0b111), y, self.get_color(color, self.bgp));
        }
    }


    fn render_bg(&mut self, rt: &mut impl Memory, display: &mut Display) {
        let tile_addr = get_tile_addr(self.x, self.scx, self.ly, self.scy);

        let bg_tilemap = get_bit(self.r_control, 3);
        let tile_id = rt.get(self.tile_offset(bg_tilemap) + tile_addr);

        let ttr = self.get_tile(tile_id, (self.ly & 0b111) + (self.scy & 0b111));

        // rendering background tile
        self.render_tile_pixel(display, rt, ttr, self.x, self.ly, self.scx);

        let window_enable = get_bit(self.r_control, 5) == 1;
        let window_visible = (0..=166).contains(&self.wx) && (0..=143).contains(&self.wy);
        let window_tilemap = get_bit(self.r_control, 6);

        if window_enable && window_visible && self.ly >= self.wy && self.x * 8 >= self.wx - 7 {
            let tile_addr = get_tile_addr(
                self.x - (self.wx - 7)/ 8,
                0,
                self.ly - self.wy,
                0,
            );

            let tile_id = rt.get(self.tile_offset(window_tilemap) + tile_addr);

            let ttr = self.get_tile(tile_id, self.ly - self.wy);
            let scx = (self.wx - 7);
            self.render_tile_pixel(display, rt, ttr, self.x, self.ly, scx);
        }
    }


    fn fetch_pixels(&mut self, rt: &mut impl Memory) {
        let window_enable = get_bit(self.r_control, 5) == 1 && self.ly >= self.wy;

        let ttr = self.bg_tile_addr(rt, self.x);
        let ttr_next = self.bg_tile_addr(rt, self.x + 1);

        let bg_fst: u16 = ((rt.get(ttr + 0) as u16) << 8) + rt.get(ttr_next + 0) as u16;
        let bg_snd: u16 = ((rt.get(ttr + 1) as u16) << 8) + rt.get(ttr_next + 1) as u16;

        let ttr = self.win_tile_addr(rt, self.x);
        let ttr_next = self.win_tile_addr(rt, self.x + 1);
        let win_fst: u16 = ((rt.get(ttr + 0) as u16) << 8) + rt.get(ttr_next + 0) as u16;
        let win_snd: u16 = ((rt.get(ttr + 1) as u16) << 8) + rt.get(ttr_next + 1) as u16;


        for i in 0..8 {
            let current_x = self.x * 8 + i;
            let px = if window_enable && current_x >= self.wx - 7 {
                let l : u8 = get_bit(win_fst, 15 - i);
                let h : u8 = get_bit(win_snd, 15 - i);
                let color = (h << 1) + l;

                FIFOPixel{
                    source: FIFOPixelSource::BACKGROUND,
                    color_id: color,
                }
            } else {
                // TODO: shift register SCX
                let l = get_bit(bg_fst, 15 - (i + (self.scx & 0b111)));
                let h = get_bit(bg_snd, 15 - (i + (self.scx & 0b111)));
                let color = (h << 1) + l;

                FIFOPixel{
                    source: FIFOPixelSource::BACKGROUND,
                    color_id: color,
                }
            };

            for s in &self.filtered_sprites {
                if s.x == current_x {

                }
            }

            self.pixel_fifo_bg.push_back(px);
        }


        for s in &self.filtered_sprites {
            for i in 0..8 {
                let current_x = self.x * 8 + i;
                if s.x - 8 >= current_x && current_x <= s.x {

                }
            }
        }
    }

    fn draw_pixels(&mut self, display: &mut Display) {
        for idx in 0..8 {
            let px = self.pixel_fifo_bg.pop_front().unwrap();

            let color = match px.source {
                FIFOPixelSource::BACKGROUND => self.get_color(px.color_id, self.bgp),
                FIFOPixelSource::WINDOW => self.get_color(px.color_id, self.bgp),
                _ => panic!("not implemented"),
            };


            display.set_pixel(self.x * 8 + idx, self.ly, color);
        }
    }

    fn render_objects(&mut self, rt: &mut impl Memory, display: &mut Display) {
        let obj_enable = get_bit(self.r_control, 1) == 1;
        if !obj_enable {
            return;
        }

        for s in self.filtered_sprites.iter().rev() {
            const DISPLAY_OFFSET: u8 = 8;
            const SPRITE_WIDTH: u8 = 8;


            let sprite_left = if s.x > DISPLAY_OFFSET {
                s.x - DISPLAY_OFFSET
            } else {
                0
            };
            let sprite_right = sprite_left + SPRITE_WIDTH;

            let cx = self.x * 8;
            let drawable = sprite_left <= cx && cx <= sprite_right;


            if drawable {
                let obj_tile_line = s.tile_line(rt, self.ly + 16 - s.y);

                let (start, end) = if sprite_right > cx {
                    (cx - sprite_left, sprite_right - cx)
                } else {
                    (0, cx - sprite_left)
                };

                self.render_obj_partial(
                    display,
                    obj_tile_line,
                    (start, end),
                    self.ly,
                    sprite_left,
                    s.palette(),
                    s.flags,
                );
            }
        }
    }

    fn bg_tile_addr(&self, rt: &impl Memory, x: u8) -> u16 {
        let tile_addr = get_tile_addr(x, self.scx, self.ly, self.scy);

        let bg_tilemap = get_bit(self.r_control, 3);
        let tile_id = rt.get(self.tile_offset(bg_tilemap) + tile_addr);

        let ttr = self.get_tile(tile_id, (self.ly & 0b111) + (self.scy & 0b111));
        return ttr;
    }

    fn win_tile_addr(&self, rt: &impl Memory, x: u8) -> u16 {
        let window_tilemap = get_bit(self.r_control, 6);

        let tile_addr = get_tile_addr(
            x - (self.wx - 7)/ 8,
            0,
            self.ly - self.wy,
            0,
        );
        let tile_id = rt.get(self.tile_offset(window_tilemap) + tile_addr);
        let ttr = self.get_tile(tile_id, self.ly - self.wy);
        return ttr;
    }

    fn render_obj_partial(
        &self,
        display: &mut Display,
        tile: (u8, u8),
        slice: (u8, u8),
        y: u8,
        x: u8,
        palette: u8,
        flags: u8,
    ) {
        let (fst, snd) = tile;
        let (t_start, t_end) = slice;

        let is_flipped_x = get_bit(flags, 5) == 1;
        let is_flipped_y = get_bit(flags, 6) == 1;
        let priority = get_bit(flags, 7);

        for i in t_start..t_end {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);

            let color = (h << 1) + l;
            if color == 0 {
                continue;
            }

            if let Some(color) = self.obj_color(color, palette) {
                let delta = if is_flipped_x { i } else { 7 - i };
                display.set_pixel(x + delta, y, color);
            }
        }
    }

    fn obj_color(&self, id: u8, palette_id: u8) -> Option<u8> {
        let palette = if palette_id == 0 {
            self.obp0
        } else {
            self.obp1
        };
        let c = self.get_color(id, palette);
        return Some(c);
    }
}

/// Calculate tile number from X, SCY, LY, SCY
fn get_tile_addr(x: u8, scx: u8, ly: u8, scy: u8) -> u16 {
    let ly = ly as u16;
    let x = x as u16;
    let scy = scy as u16;
    let scx = scx as u16;

    let tile_x = (x + (scx >> 3)) & 0x1F;
    let tile_y = ((scy + ly) >> 3) & 0x1F;

    let tile_no = tile_x + (tile_y * 32);

    assert!(tile_no <= 0x3FF);
    return tile_no;
}

pub struct Display {
    pixels: Vec<u8>,
    width: u8,
    height: u8,
}

impl Display {
    pub fn from(canvas: &Canvas<sdl2::video::Window>) -> Display {
        let (width, height) = canvas.window().drawable_size();

        let size = width as usize * height as usize;
        Display {
            pixels: vec![0; size],
            width: width as u8,
            height: height as u8,
        }
    }

    fn get_pixel(&self, x: u8, y: u8) -> u8 {
        let addr = x as usize + y as usize * self.width as usize;
        return self.pixels[addr];
    }

    fn set_pixel(&mut self, x: u8, y: u8, color: u8) {
        let addr = x as usize + y as usize * self.width as usize;
        let max_addr = self.width as usize * self.height as usize;

        if addr < max_addr {
            self.pixels[addr] = color;
        }
    }

    fn get_color(&mut self, x: u8, y: u8) -> Color {
        return color_from_code(self.get_pixel(x, y) as usize);
    }
    pub fn render(&mut self, canvas: &mut Canvas<sdl2::video::Window>) {
        let mut rect = Rect::new(0, 0, 1, 1);
        for y in 0..self.height {
            for x in 0..self.width {
                canvas.set_draw_color(self.get_color(x, y));
                rect.x = x as i32;
                rect.y = y as i32;
                canvas.fill_rect(rect).unwrap();
            }
        }
    }

}

fn tile_addr(tile_id: u8, signed_mode: bool) -> u16 {
    let b0 = 0x8000;
    let b1 = 0x8800;
    let b2 = 0x9000;

    let select = get_bit(tile_id, 7) == 1;

    let mut tile = tile_id;

    if select {
        if signed_mode {
            tile = !(tile - 1);
            tile = 0x80 - tile;
        } else {
            tile &= 0x7F
        }
    }

    let base = match (signed_mode, select) {
        (false, false) => b0,
        (false, true) => b1,

        (true, false) => b2,
        (true, true) => b1,
    };

    let block_addr = tile as u16 * 16;
    return base + block_addr;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signed_mode_128_returns_8800() {
        let got = tile_addr(128, true);
        assert_eq!(b64(got), "8800");
    }

    #[test]
    fn test_signed_mode_0_returns_9000() {
        let got = tile_addr(0, true);
        assert_eq!(b64(got), "9000");
    }

    #[test]
    fn test_signed_mode_127_returns_last_addr_of_block_2() {
        let got = tile_addr(127, true);
        assert_eq!(b64(got), "97F0");
    }

    #[test]
    fn test_signed_mode_255_returns_last_addr_of_block_1() {
        let got = tile_addr(0xFF, true);
        assert_eq!(b64(got), "8FF0");
    }
}

