pub trait Rom<'b> {
    fn get(&self, addr: u16) -> u8;
    fn set(&mut self, addr: u16, val: u8);
}

pub struct RomNoMBC<'a> {
    pub rom: &'a Vec<u8>,
}

impl Rom<'_> for RomNoMBC<'_> {
    fn get(&self, addr: u16) -> u8 {
        self.rom[addr as usize]
    }
    fn set(&mut self, addr: u16, val: u8) {}
}

pub struct RomMBC3<'a> {
    rom: &'a Vec<u8>,
    rom_bank: u8,
    exram_enable: bool,
    ram_bank: u8,
    rtc_reg: u8,
}

impl RomMBC3<'_> {
    pub fn new<'a>(rom: &'a Vec<u8>) -> RomMBC3<'a> {
        return RomMBC3 {
            rom,
            rom_bank: 1,
            exram_enable: false,
            rtc_reg: 0,
            ram_bank: 0,
        };
    }
}

impl Rom<'_> for RomMBC3<'_> {
    fn set(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // rom enable
                if val & 0xA == 0xA {
                    self.exram_enable = true;
                    // should be disabled after access
                } else {
                    self.exram_enable = false;
                }
            }
            0x2000..=0x3FFF => {
                // change rom bank
                self.rom_bank = val & 0x7F;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            }
            0x4000..=0x5FFF => {
                // ram bank or rtc register select
                if val <= 0x03 {
                    self.ram_bank = val;
                    self.rtc_reg = 0;
                }
                else if val >= 0x08 && val <= 0x0C {
                    self.rtc_reg = val;
                    self.ram_bank = 0;
                }
            }
            0x6000..=0x7FFF => {
                // latch clock data
            }
            _ => {}
        }
    }

    fn get(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom[addr as usize],
            0x4000..=0x7FFF => {
                let rom_addr = addr as usize - 0x4000 + 0x4000 * self.rom_bank as usize;
                self.rom[rom_addr]
            }
            0xA000..=0xBFFF => {
                // select ram bank or the rtc_reg
                0
            }
            _ => {
                panic!("not implemented")
            }
        }
    }
}
