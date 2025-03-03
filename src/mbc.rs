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
    bank_or_rtc: u8,
    ram: Vec<Vec<u8>>,
}

impl RomMBC3<'_> {
    pub fn new<'a>(rom: &'a Vec<u8>) -> RomMBC3<'a> {
        return RomMBC3 {
            rom,
            rom_bank: 1,
            exram_enable: false,
            bank_or_rtc: 0,
            ram: vec![vec![0; 0x2000]; 4],
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
                self.bank_or_rtc = val;
            }
            0x6000..=0x7FFF => {
                // latch clock data
            }
            0xA000..=0xBFFF => {
                if self.bank_or_rtc <= 3 {
                    self.ram[self.bank_or_rtc as usize][addr as usize - 0xA000] = val;
                }
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
                if self.bank_or_rtc <= 3 {
                    return self.ram[self.bank_or_rtc as usize][addr as usize - 0xA000];
                } else if self.bank_or_rtc >= 8 && self.bank_or_rtc <= 0x0C {
                }
                0
            }
            _ => {
                panic!("not implemented")
            }
        }
    }
}
