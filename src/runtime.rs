use std::io::Read;
use std::mem::size_of;
use crate::byteop::*;

#[derive(Debug)]
struct CpuRegisters {
    ra: u8,
    rf: u8,
    rb: u8,
    rc: u8,
    rd: u8,
    re: u8,
    rh: u8,
    rl: u8,
    sp: u16,
    pc: u16,
}
enum CFlag {
    Z = 7,  // zero
    S = 6,  // subtraction
    H = 5,  // half carry
    CY = 4, // carry
}

impl CpuRegisters {
    fn set_flag(&mut self, flag: CFlag, val: u8) {
        self.rf = set_bit(self.rf, flag as u8, val == 1);
    }
    fn get_flag(&self, flag: CFlag) -> u8 {
        return get_bit(self.rf, flag as u8);
    }

    fn bc(&self) -> u16 {
        return join_u8(self.rb, self.rc);
    }
    fn set_bc(&mut self, val: u16) {
        let (h, l) = split_u16(val);
        self.rb = h;
        self.rc = l;
    }

    fn de(&self) -> u16 {
        return join_u8(self.rd, self.re);
    }
    fn set_de(&mut self, val: u16) {
        let (h, l) = split_u16(val);
        self.rd = h;
        self.re = l;
    }

    fn hl(&self) -> u16 {
        return join_u8(self.rh, self.rl);
    }
    fn set_hl(&mut self, val: u16) {
        let (h, l) = split_u16(val);
        self.rh = h;
        self.rl = l;
    }
}

pub struct Runtime<'a> {
    cpu: CpuRegisters,
    rom: &'a Vec<u8>,
    bootstrap: &'a Vec<u8>,
    pub vram: Vec<u8>,
    wram: Vec<u8>,
}

impl Runtime<'_> {
    pub fn load<'a>(bootstrap: &'a Vec<u8>, rom: &'a Vec<u8>) -> Runtime<'a> {
        return Runtime {
            cpu: CpuRegisters {
                ra: 0,
                rf: 0,
                rb: 0,
                rc: 0,
                rd: 0,
                re: 0,
                rh: 0,
                rl: 0,
                pc: 0,
                sp: 0,
            },
            rom,
            bootstrap,
            vram: vec![0; 0x9fff - 0x8000 + 1],
            wram: vec![0; 0xffff - 0x8000 + 1],
        };
    }

    fn next_opcode(&mut self) -> u8 {
        let opcode = self.get(self.cpu.pc);
        self.cpu.pc += 1;
        return opcode;
    }

    pub fn tick(&mut self) -> u8 {
        if self.cpu.pc == 0x2817 {
            panic!("YEY");
        }
        println!(
            "{} - Opcode {}: {:?}",
            b64(self.cpu.pc),
            b64(self.get(self.cpu.pc)),
            self.cpu
        );
        let opcode = self.next_opcode();

        // https://meganesu.github.io/generate-gb-opcodes/
        return match opcode {
            0x00 => {
                // NOP
                1
            }
            0x01 => {
                // LD BC, u16
                self.cpu.rc = self.next_opcode();
                self.cpu.rb = self.next_opcode();
                3
            }
            0x03 => {
                self.cpu.set_bc(self.cpu.bc() + 1);
                2
            }
            0x04 => {
                self.cpu.rb += 1;
                self.cpu.set_flag(CFlag::Z, (self.cpu.rb == 0) as u8);
                self.cpu.set_flag(CFlag::S, 0);
                1
            }
            0x05 => {
                self.cpu.rb = self.cpu.rb.wrapping_sub(1u8);
                self.cpu.set_flag(CFlag::Z, (self.cpu.rb == 0) as u8);
                self.cpu.set_flag(CFlag::S, 1);
                1
            }
            0x06 => {
                self.cpu.rb = self.next_opcode();
                2
            }
            0x08 => {
                let (h, l) = split_u16(self.cpu.sp);
                let al = self.next_opcode();
                let ah = self.next_opcode();
                let addr = join_u8(ah, al);
                self.set(addr, l);
                self.set(addr + 1, h);
                5
            }
            0x0C => {
                // INC CY
                self.cpu.rc += 1;
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::Z, (self.cpu.rc == 0) as u8);
                1
            }
            0x0D => {
                self.cpu.rc -= 1;
                self.cpu.set_flag(CFlag::Z, (self.cpu.rc == 0) as u8);
                self.cpu.set_flag(CFlag::S, 1);
                1
            }
            0x0E => {
                // LD CY, u8
                self.cpu.rc = self.next_opcode();
                2
            }
            0x0F => {
                let lsb = self.cpu.ra & 0b1;
                let cy = self.cpu.get_flag(CFlag::CY);
                self.cpu.ra = (self.cpu.ra >> 1) + (cy << 7);
                self.cpu.set_flag(CFlag::CY, lsb);

                self.cpu.set_flag(CFlag::Z, 0);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::H, 0);
                1
            }
            0x10 => {
                panic!("STOP!");
            }
            0x11 => {
                // LD DE, u16
                self.cpu.re = self.next_opcode();
                self.cpu.rd = self.next_opcode();
                3
            }
            0x12 => {
                self.set(self.cpu.de(), self.cpu.ra);
                2
            }
            0x13 => {
                self.cpu.set_de(self.cpu.de() + 1);
                2
            }
            0x14 => {
                self.cpu.rd += 1;
                1
            }
            0x15 => {
                self.cpu.rd -= 1;
                self.cpu.set_flag(CFlag::Z, (self.cpu.rd == 0) as u8);
                self.cpu.set_flag(CFlag::S, 1);
                1
            }
            0x16 => {
                self.cpu.rd = self.next_opcode();
                2
            }
            0x17 => {
                let msb = (self.cpu.ra & (1 << 7)) >> 7;

                self.cpu.ra = self.cpu.ra << 1 + self.cpu.get_flag(CFlag::CY);
                self.cpu.set_flag(CFlag::CY, msb);
                1
            }
            0x18 => {
                let raddr = self.next_opcode() as i8;
                self.cpu.pc = self.cpu.pc.wrapping_add(raddr as u16);
                3
            }
            0x1A => {
                self.cpu.ra = self.get(self.cpu.de());
                2
            }
            0x3B => {
                self.cpu.sp -= 1;
                2
            }
            0x1D => {
                self.cpu.re -= 1;
                self.cpu.set_flag(CFlag::Z, (self.cpu.re == 0) as u8);
                self.cpu.set_flag(CFlag::S, 1);
                1
            }
            0x1E => {
                self.cpu.re = self.next_opcode();
                2
            }
            0x20 => {
                let addr = self.next_opcode() as i8;
                if self.cpu.get_flag(CFlag::Z) == 0 {
                    self.cpu.pc = self.cpu.pc.wrapping_add(addr as u16);
                    3
                } else {
                    2
                }
            }
            0x21 => {
                self.cpu.rl = self.next_opcode();
                self.cpu.rh = self.next_opcode();
                3
            }
            0x22 => {
                self.set(self.cpu.hl(), self.cpu.ra);
                self.cpu.set_hl(self.cpu.hl() + 1);
                2
            }
            0x23 => {
                self.cpu.set_hl(self.cpu.hl() + 1);
                2
            }
            0x24 => {
                self.cpu.rh = self.cpu.rh.wrapping_add(1);
                self.cpu.set_flag(CFlag::Z, (self.cpu.rh == 0) as u8);
                self.cpu.set_flag(CFlag::S, 0);
                1
            }
            0x28 => {
                let raddr = self.next_opcode() as i8;
                if self.cpu.get_flag(CFlag::Z) == 1 {
                    self.cpu.pc = self.cpu.pc.wrapping_add(raddr as u16);
                    3
                } else {
                    2
                }
            }
            0x2E => {
                self.cpu.rl = self.next_opcode();
                2
            }
            0x31 => {
                // jr nc, s8
                let b0 = self.next_opcode();
                let b1 = self.next_opcode();
                self.cpu.sp = ((b1 as u16) << 8) + (b0 as u16);
                3
            }
            0x32 => {
                let hl = self.cpu.hl();
                self.set(hl, self.cpu.ra);
                self.cpu.set_hl(hl -1);
                2
            }
            0x3E => {
                self.cpu.ra = self.next_opcode();
                2
            }
            0x3D => {
                self.cpu.ra -= 1;
                self.cpu.set_flag(CFlag::S, 1);
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                1
            }
            0x4B => {
                self.cpu.rc = self.cpu.rb;
                1
            }
            0x4C => {
                self.cpu.rc = self.cpu.rh;
                1
            }
            0x4F => {
                self.cpu.rc = self.cpu.ra;
                1
            }
            0x57 => {
                self.cpu.rd = self.cpu.ra;
                1
            }
            0x58 => {
                self.cpu.rb = self.cpu.re;
                1
            }
            0x59 => {
                self.cpu.re = self.cpu.rc;
                1
            }
            0x6C => {
                self.cpu.rl = self.cpu.rh;
                1
            }
            0x67 => {
                self.cpu.rh = self.cpu.ra;
                1
            }
            0x68 => {
                self.cpu.rl = self.cpu.rb;
                1
            }
            0x69 => {
                self.cpu.rl = self.cpu.rc;
                1
            }
            0x6A => {
                self.cpu.rl = self.cpu.rd;
                1
            }
            0x73 => {
                self.set(self.cpu.hl(), self.cpu.re);
                2
            }
            0x77 => {
                self.set(self.cpu.hl(), self.cpu.ra);
                2
            }
            0x78 => {
                self.cpu.ra = self.cpu.rb;
                1
            }
            0x79 => {
                self.cpu.ra = self.cpu.rc;
                1
            }
            0x7A => {
                self.cpu.ra = self.cpu.rd;
                1
            }
            0x7B => {
                self.cpu.ra = self.cpu.re;
                1
            }
            0x7C => {
                self.cpu.ra = self.cpu.rh;
                1
            }
            0x7D => {
                self.cpu.ra = self.cpu.rl;
                1
            }
            0x7E => {
                self.cpu.ra = self.get(self.cpu.hl());
                2
            }
            0x83 => {
                self.cpu.ra = self.cpu.re | self.cpu.ra;
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::CY, 0);
                self.cpu.set_flag(CFlag::H, 0);
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                1
            }
            // 0x7F => {
            //     self.cpu.ra = self.cpu.ra;
            //     1
            // }
            0x86 => {
                self.cpu.ra = self.get(self.cpu.hl()) + self.cpu.ra;
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                2
            }
            0x88 => {
                self.cpu.ra += self.cpu.rb + self.cpu.get_flag(CFlag::CY);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                1
            }
            0x90 => {
                let cy = self.cpu.ra < self.cpu.rb;
                self.cpu.ra = self.cpu.ra.wrapping_sub(self.cpu.rb);
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                self.cpu.set_flag(CFlag::S, 1);
                self.cpu.set_flag(CFlag::CY, cy as u8);
                1
            }
            0xAF => {
                self.cpu.ra ^= self.cpu.ra;

                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                1
            }
            0xBE => {
                let val = self.get(self.cpu.hl());
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == val) as u8);
                self.cpu.set_flag(CFlag::S, 1);
                2
            }
            0xC1 => {
                self.cpu.rc = self.stack_pop();
                self.cpu.rb = self.stack_pop();
                3
            }
            0xC3 => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.cpu.pc = join_u8(h, l);
                4
            }
            0xC5 => {
                self.stack_push_u16(self.cpu.bc());
                4
            }
            0xC9 => {
                self.cpu.pc = self.stack_pop_u16();
                4
            }
            0xCB => {
                let opnext = self.next_opcode();
                self.eval_cb(opnext)
            }
            0xCD => {
                // call u16
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.stack_push_u16(self.cpu.pc); // save PC to the stack
                self.cpu.pc = join_u8(h, l);

                6
            }
            0xE0 => {
                let addr = 0xFF00 + self.next_opcode() as u16;
                self.set(addr, self.cpu.ra);
                3
            }
            0xE2 => {
                let addr = 0xFF00 + self.cpu.rc as u16;
                self.set(addr, self.cpu.ra);
                2
            }
            0xEA => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                let addr = join_u8(h, l);
                self.set(addr, self.cpu.ra);
                4
            }
            0xF0 => {
                let addr = 0xFF00 + self.next_opcode() as u16;
                self.cpu.ra = self.get(addr);
                3
            }
            0xFE => {
                let imm = self.next_opcode();
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == imm) as u8);
                self.cpu.set_flag(CFlag::S, 1);

                2
            }
            _ => {
                panic!("ERROR: Opcode 0x{} not implemented!", b64(opcode));
            }
        };
    }

    fn eval_cb(&mut self, opcode: u8) -> u8 {
        return match opcode {
            0x11 => {
                let c = self.cpu.rc;
                self.cpu.rc = (c << 1) + self.cpu.get_flag(CFlag::CY);
                2
            }
            0x7C => {
                let msb = get_bit(self.cpu.rh, 7);
                self.cpu.set_flag(CFlag::Z, msb ^ 0b1);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::H, 1);
                2
            }

            _ => {
                panic!("ERROR: Opcode CB{} not implemented", b64(opcode));
            }
        };
    }

    fn boot_rom_disabled(&self) -> bool {
        return self.get(0xFF50) == 1
    }
    // RAM
    pub fn get(&self, addr: u16) -> u8 {
        return match addr {
            0x0000..=0x00FF => {
                if self.boot_rom_disabled() {
                    self.rom[addr as usize]
                } else {
                    self.bootstrap[(addr - 0x000) as usize]
                }
            }
            0x0100..=0x3FFF => {
                if self.boot_rom_disabled() {
                    self.rom[addr as usize]
                } else {
                    self.rom[(addr - 0x100) as usize]
                }
            }
            0x4000..=0x7FFF => {
                if self.boot_rom_disabled() {
                    self.rom[addr as usize]
                } else {
                    self.rom[(addr - 0x0100) as usize]
                }
            }
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xFFFF => self.wram[(addr - 0xA000) as usize],
            _ => {
                panic!("Memory access out of bounds! {}", b64(addr));
            }
        };
    }

    pub fn set(&mut self, addr: u16, val: u8) -> () {
        match addr {
            0x0000..=0x3FFF => {
                panic!("Write on RO memory ({}): {}", b64(addr), b64(val));
            }
            0x7FFF => {
                panic!("DANGER!");
            }
            0x8000..=0x9FFF => {
                println!("Setting memory region {}", b64(addr));
                self.vram[(addr - 0x8000) as usize] = val
            },
            0xA000..=0xFFFF => self.wram[(addr - 0xA000) as usize] = val,
            _ => {
                // panic!("Access to unknown memory region {}", addr)
            }
        }
    }

    // STACK
    fn stack_push_u16(&mut self, value: u16) {
        let (h, l) = split_u16(value);
        self.stack_push(h);
        self.stack_push(l);
    }

    fn stack_push(&mut self, value: u8) {
        self.cpu.sp -= 1;
        self.set(self.cpu.sp, value);
    }
    fn stack_pop(&mut self) -> u8 {
        let val = self.get(self.cpu.sp);
        self.cpu.sp += 1;
        return val;
    }
    fn stack_pop_u16(&mut self) -> u16 {
        let l = self.stack_pop();
        let h = self.stack_pop();
        return join_u8(h, l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_returns_b64_numbers() {
        assert_eq!(b64(0xFFu16), "0x00FF".to_string());
        assert_eq!(b64(0x3Au16), "0x003A".to_string());
    }
}
