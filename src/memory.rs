use crate::{byteop::*, mbc::Rom};

pub trait Memory {
    fn get(&self, addr: u16) -> u8;
    fn set(&mut self, addr: u16, value: u8);
    fn hwset(&mut self, addr: u16, value: u8);
}

#[derive(Debug)]
pub enum HWInput {
    BtnA      = 0b100,
    BtnB      = 0b101,
    BtnSelect = 0b110,
    BtnStart  = 0b111,

    ArrRight  = 0b000,
    ArrLeft   = 0b001,
    ArrUp     = 0b010,
    ArrDown   = 0b011,
}


pub struct MMU<'a> {
    boot_rom: &'a Vec<u8>,
    rom: &'a mut dyn Rom<'a>,

    vram: Vec<u8>,
    wram: Vec<u8>,
    hwcfg: u8,

    inputs: u8,
}

impl MMU<'_> {
    pub fn new<'a>(boot_rom: &'a Vec<u8>, rom: &'a mut dyn Rom<'a>) -> MMU<'a> {
        let hwcfg = rom.get(0x147);

        MMU {
            boot_rom,
            rom,
            hwcfg,
            vram: vec![0; 0x9fff - 0x8000 + 1],
            wram: vec![0; 0xffff - 0x8000 + 1],
            inputs: 0xFF,
        }
    }

    fn boot_rom_disabled(&self) -> bool {
        return self.get(0xFF50) == 1;
    }

    pub fn press(&mut self, btn: HWInput, pressed: bool) {
        let addr = btn as u8;
        self.inputs = set_bit(self.inputs, addr, !pressed);
    }

    // TODO: transfer take 160 machine cycles: 640 dots
    fn dma(&mut self, addr: u8) {
        let source = (addr as u16) << 8;
        let dest = 0xFE00;

        for i in 0x00..=0x9Fu16 {
            let byte = self.get(source + i);
            self.set(dest + i, byte);
        }
    }
}

impl Memory for MMU<'_> {
    fn get(&self, addr: u16) -> u8 {
        return match addr {
            0x0000..=0x00FF => {
                if self.boot_rom_disabled() {
                    self.rom.get(addr)
                } else {
                    self.boot_rom[addr as usize]
                }
            }
            0x0100..=0x3FFF => self.rom.get(addr),
            0x4000..=0x7FFF => self.rom.get(addr),

            0x8000..=0x9FFF => {
                // rom + offset
                self.vram[(addr - 0x8000) as usize]
            }

            0xE000..=0xFDFF => {
                // mirror of 0xCD00-0xDDFF
                self.wram[addr as usize - 0xA000 - 0x2000]
            }

            0xFF00 => {
                let read_mask = self.wram[addr as usize - 0xA000];
                get_inputs(read_mask, self.inputs)
            }
            0xA000..=0xBFFF => self.rom.get(addr),
            0xC000..=0xFFFF => self.wram[(addr - 0xA000) as usize],
            _ => {
                panic!("Memory access out of bounds! {}", b64(addr));
            }
        };
    }

    fn hwset(&mut self, addr: u16, val: u8) -> () {
        match addr {
            0xFF26 => {
                self.wram[addr as usize - 0xA000] = val
            }
            0xFF04 => {
                self.wram[addr as usize - 0xA000] = val
            }
            _ => panic!("Unhandled address {} for hwset", b64(addr)),
        }
    }

    fn set(&mut self, addr: u16, val: u8) -> () {
        match addr {
            0x0000..=0x7FFF => {
                self.rom.set(addr, val);
            }
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
            0xE000..=0xFDFF => {
                // mirror of 0xCD00-0xDDFF
                self.wram[addr as usize - 0xA000 - 0x2000] = val;
            }
            0xFF00 => {
                let read_mask = (val & 0x30) + 0x60;
                self.wram[addr as usize - 0xA000] = read_mask;
            }
            0xFF26 => {
                // only the first bit of this register can be set by games,
                // this register can only be modified via hwset
                let current_value = self.get(0xFF26) ;
                let new_value: u8 = set_bit(
                    current_value,
                    7,
                    get_bit(val, 7) == 1,
                );
                self.hwset(addr, new_value);
            }
            0xFF04 => {
                self.hwset(addr, 0);
            }
            0xFF46 => {
                self.dma(val);
            }
            0xA000..=0xBFFF => self.rom.set(addr, val),
            0xC000..=0xFFFF => {
                self.wram[(addr - 0xA000) as usize] = val
            },
            _ => {
                // panic!("Access to unknown memory region {}", addr)
            }
        }
    }
}

fn get_inputs(mask: u8, inputs: u8) -> u8 {
    let upper = (mask & 0x30) + 0x60;
    let lower = if get_bit(upper, 4) == 0 {
        // dpad
        inputs & 0b1111
    } else if get_bit(upper, 5) == 0 {
        inputs >> 4
    } else {
        0xF
    };

    return upper + lower;
}
