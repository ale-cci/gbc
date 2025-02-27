use crate::byteop::*;
use crate::memory::Memory;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
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

    sprites_line_counter: u8,
    remaining_cycles: u8,
    wait: u16,
}
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

    fn update(&mut self, rt: &impl Memory) {
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
            sprites_line_counter: 0,
            remaining_cycles: 0,
            wait: 0,
            bgp: 0,
            sprites,
        }
    }

    fn get_color(&self, id: u8, palette: u8) -> u8 {
        let shift = id * 2;
        let color = (self.bgp & (0b11 << shift)) >> shift;

        return color;
    }

    pub fn update(&mut self, rt: &mut impl Memory, cc: u8, display: &mut Display) {
        self.r_control = rt.get(0xFF40);
        self.r_status = rt.get(0xFF41);
        self.scy = rt.get(0xFF42);
        self.scx = rt.get(0xFF43);
        self.ly = rt.get(0xFF44);
        self.lyc = rt.get(0xFF45); // 0..=153
        self.bgp = rt.get(0xFF47);
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
            for s in &mut self.sprites {
                s.update(rt);
            }
            self.render(rt, display);
        }
    }

    // render background
    fn render(&mut self, rt: &mut impl Memory, display: &mut Display) {
        let tile_addr = get_tile_addr(self.x, self.scx, self.ly, self.scy);

        let bg_tilemap = get_bit(self.r_control, 3);
        let tile_id = rt.get(self.tile_offset(bg_tilemap) + tile_addr);

        let ttr = self.get_tile(tile_id, (self.ly & 0b111) + (self.scy & 0b111));

        // rendering background tile
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

        let obj_size = get_bit(self.r_control, 2);

        if self.sprites_line_counter < 10 {
            for s in &self.sprites {
                const DISPLAY_OFFSET: u8 = 8;
                const SPRITE_WIDTH: u8 = 8;

                if s.is_visible(self.ly, obj_size) {
                    let sprite_left = if s.x > DISPLAY_OFFSET {
                        s.x - DISPLAY_OFFSET
                    } else {
                        0
                    };
                    let sprite_right = sprite_left + SPRITE_WIDTH;

                    let cx = self.x * 8;
                    let drawable = sprite_left <= cx && cx <= sprite_right;

                    if drawable {
                        if sprite_right <= self.x {
                            self.sprites_line_counter += 1;
                        }
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
                        );
                    }
                }

                if self.sprites_line_counter == 10 {
                    // n. of sprites
                    break;
                }
            }
        }

        self.x += 1;
        if self.x == 20 {
            self.x = 0; // hblank
            self.wait = 456;
            self.ly += 1;
            self.sprites_line_counter = 0;

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

    /// Calculates the memory address of the tile, given SCY & tile_numbere
    fn get_tile(&self, tile: u8, scy: u8) -> u16 {
        let mode_8800 = get_bit(self.r_control, 4) == 0;

        let intratile = (scy & 0b111) as u16;
        return tile_addr(tile, mode_8800) + (intratile * 2);
    }

    fn render_tile(&self, display: &mut Display, rt: &impl Memory, ttr: u16, x: u8, y: u8) {
        let fst = rt.get(ttr + 0);
        let snd = rt.get(ttr + 1);

        for i in 0..8 {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);
            let color = (h << 1) + l;

            let x = x * 8 + (7 - i);
            let y = y;

            display.set_pixel(x, y, self.get_color(color, self.bgp));
        }
    }

    fn render_obj_partial(
        &self,
        display: &mut Display,
        tile: (u8, u8),
        slice: (u8, u8),
        y: u8,
        x: u8,
        palette: u8,
    ) {
        let (fst, snd) = tile;
        let (t_start, t_end) = slice;

        for i in t_start..t_end {
            let l = get_bit(fst, i);
            let h = get_bit(snd, i);

            let color = (h << 1) + l;

            if let Some(color) = self.obj_color(color, palette) {
                display.set_pixel(x + (7 - i), y, color);
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

    if tile_no > 0x3FF {
        println!("chuckle: i'm in danger");
    }
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
