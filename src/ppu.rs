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
    bgp: u8,

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
            bgp: 0,
        }
    }
    fn get_color(&self, id: u8) -> Color {
        let shift = id * 2;
        let color = (self.bgp & (0b11 << shift)) >> shift;

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
        return colors[color as usize];
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
            println!("Window enable???");
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

            display.set_pixel(x, y, self.get_color(color));
        }
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

fn tile_addr(tile_id: u8, signed_mode: bool) -> u16 {
    let b0 = 0x8000;
    let b1 = 0x8800;
    let b2 = 0x9000;


    let select = get_bit(tile_id, 7) == 1;

    let mut tile = tile_id;

    if select {
        if signed_mode {
            tile = !(tile -1);
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
