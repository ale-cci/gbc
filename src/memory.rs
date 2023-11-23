use crate::byteop::*;

pub trait Memory {
    fn get(&self, addr: u16) -> u8;
    fn set(&mut self, addr: u16, value: u8);
}

pub struct MMU<'a> {
    boot_rom: &'a Vec<u8>,
    rom: &'a Vec<u8>,
    vram: Vec<u8>,
    wram: Vec<u8>,
    hwcfg: u8,
}

impl MMU<'_> {
    pub fn new<'a>(boot_rom: &'a Vec<u8>, rom: &'a Vec<u8>) -> MMU<'a> {
        MMU {
            boot_rom,
            rom,
            hwcfg: rom[0x147],
            vram: vec![0; 0x9fff - 0x8000 + 1],
            wram: vec![0; 0xffff - 0x8000 + 1],
        }
    }

    fn boot_rom_disabled(&self) -> bool {
        return self.get(0xFF50) == 1;
    }
}

impl Memory for MMU<'_> {
    fn get(&self, addr: u16) -> u8 {
        return match addr {
            0x0000..=0x00FF => {
                if self.boot_rom_disabled() {
                    self.rom[addr as usize]
                } else {
                    self.boot_rom[addr as usize]
                }
            }
            // 0xFF44 => { 0x90 }
            0x0100..=0x3FFF => self.rom[addr as usize],
            0x4000..=0x7FFF => self.rom[addr as usize],
            0x8000..=0x9FFF => {
                // rom + offset
                self.vram[(addr - 0x8000) as usize]
            }
            // 0xE000..=0xFDFF => {
            //     self.wram[addr as usize - 0xA000 - 0x2000]
            // }
            0xA000..=0xFFFF => self.wram[(addr - 0xA000) as usize],
            _ => {
                panic!("Memory access out of bounds! {}", b64(addr));
            }
        };
    }

    fn set(&mut self, addr: u16, val: u8) -> () {
        match addr {
            0x0000..=0x3FFF => {
                println!("Write on RO memory ({}): {}", b64(addr), b64(val));
            }
           //  0xE000..=0xFDFF => {
           //      self.wram[addr as usize - 0xA000 - 0x2000] = val;
            // }
            0x7FFF => {
                // panic!("DANGER!");
            }
            0xFF04 => {
                self.wram[addr as usize - 0xA000] = 0;
            }
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
            0xA000..=0xFFFF => self.wram[(addr - 0xA000) as usize] = val,
            _ => {
                // panic!("Access to unknown memory region {}", addr)
            }
        }
    }

}
