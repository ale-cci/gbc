use crate::{byteop::*, registers};
use crate::mbc::Rom;
use crate::memory::{HWInput, Memory, MMU};
use crate::registers::IF;
use crate::timer::Timer;
use std::fmt;

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

    ime: bool,
    debug: bool,
    halt: bool,
}

enum CFlag {
    Z = 7,  // zero
    S = 6,  // subtraction
    H = 5,  // half carry
    CY = 4, // carry
}

impl fmt::Debug for CpuRegisters {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "A:{ra} F:{rf} B:{rb} C:{rc} D:{rd} E:{re} H:{rh} L:{rl} SP:{sp} PC:{pc}",
            ra = b64(self.ra),
            rf = b64(self.rf),
            rb = b64(self.rb),
            rc = b64(self.rc),
            rd = b64(self.rd),
            re = b64(self.re),
            rh = b64(self.rh),
            rl = b64(self.rl),
            sp = b64(self.sp),
            pc = b64(self.pc),
        )
    }
}

impl CpuRegisters {
    fn new() -> CpuRegisters {
        CpuRegisters {
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
            ime: false,
            debug: false,
            halt: false,
        }
    }

    fn atboot() -> CpuRegisters {
        CpuRegisters {
            ra: 0x01,
            rf: 0b10110000,
            rb: 0x00,
            rc: 0x13,
            rd: 0x00,
            re: 0xD8,
            rh: 0x01,
            rl: 0x4D,
            pc: 0x0100,
            sp: 0xFFFE,
            ime: false,
            debug: true,
            halt: false,
        }
    }
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

    fn af(&self) -> u16 {
        return join_u8(self.ra, self.rf);
    }

    fn daa(&mut self) {
        // blindly implemented & tested following:
        // https://ehaskins.com/2018-01-30%20Z80%20DAA/
        // https://forums.nesdev.org/viewtopic.php?t=15944
        let h_flag = self.get_flag(CFlag::H) == 1;
        let c_flag = self.get_flag(CFlag::CY) == 1;
        let n_flag = self.get_flag(CFlag::S) == 1;

        let mut cy = c_flag as u8;
        if !n_flag {
            if c_flag || self.ra > 0x99 {
                self.ra = self.ra.wrapping_add(0x60);
                cy = 1;
            }
            if h_flag || ((self.ra & 0xF) > 0x9) {
                self.ra = self.ra.wrapping_add(0x6);
            }
        } else {
            if c_flag {
                self.ra = self.ra.wrapping_sub(0x60);
            }
            if h_flag {
                self.ra = self.ra.wrapping_sub(0x6);
            }
        }

        self.set_flag(CFlag::Z, (self.ra == 0) as u8);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, cy);
    }

    fn set_af(&mut self, val: u16) {
        let (h, l) = split_u16(val);
        self.ra = h;
        self.rf = l & 0xF0;
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

    fn rlc(&mut self, val: u8) -> u8 {
        let msb = get_bit(val, 7);
        let res = (val << 1) + msb;

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, msb);
        return res;
    }

    fn rl(&mut self, val: u8) -> u8 {
        let msb = get_bit(val, 7);
        let res = (val << 1) + self.get_flag(CFlag::CY);

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, msb);
        return res;
    }
    fn sla(&mut self, val: u8) -> u8 {
        let msb = get_bit(val, 7);
        let res = val << 1;

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, msb);
        return res;
    }

    fn sra(&mut self, val: u8) -> u8 {
        let lsb = get_bit(val, 0);
        let b7 = get_bit(val, 7);
        let res = (val >> 1) + (b7 << 7);

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, lsb);
        return res;
    }

    fn bit(&mut self, val: u8, pos: u8) {
        let value = get_bit(val, pos);
        self.set_flag(CFlag::Z, value ^ 0x1);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 1);
    }

    fn add_u16_i8(&mut self, a: u16, b: i8) -> u16 {
        let b = b as u16;
        let res = a.wrapping_add(b as u16);

        let cy = ((a & 0xFF) + (b & 0xFF)) & 0x100 == 0x100;
        let hc = ((a & 0xF) + (b & 0xF)) & 0x10 == 0x10;

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, hc as u8);
        self.set_flag(CFlag::CY, cy as u8);
        return res;
    }

    fn srl(&mut self, val: u8) -> u8 {
        let lsb = val & 0x1;
        let val = val >> 1;

        self.set_flag(CFlag::Z, (val == 0) as u8);
        self.set_flag(CFlag::CY, lsb);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::S, 0);

        return val;
    }

    fn dec(&mut self, val: u8) -> u8 {
        let res = val.wrapping_sub(1);
        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 1);
        self.set_flag(CFlag::H, (val & 0b1111 == 0b0000) as u8);
        return res;
    }

    // cpu arithmetic instructions
    fn add_ra(&mut self, val: u8) -> u8 {
        // https://gist.github.com/meganesu/9e228b6b587decc783aa9be34ae27841
        let half_carry = (((self.ra & 0xF) + (val & 0xF)) & 0x10) == 0x10;

        let res = self.ra as u16 + val as u16;

        self.ra = (res & 0b11111111) as u8;
        self.set_flag(CFlag::Z, (self.ra == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::CY, get_bit(res, 8));
        self.set_flag(CFlag::H, half_carry as u8);
        return 1;
    }

    fn sub_ra(&mut self, val: u8) -> u8 {
        let res = self.ra.wrapping_sub(val);

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 1);
        self.set_flag(CFlag::CY, (self.ra < val) as u8);
        let h = (self.ra & 0b1111) < (val & 0b1111);
        self.set_flag(CFlag::H, h as u8);
        return res;
    }

    fn sbc_ra(&mut self, val: u8) -> u8 {
        let res = self.ra.wrapping_sub(val);

        let cy = self.get_flag(CFlag::CY);
        let res = res.wrapping_sub(cy);

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 1);
        self.set_flag(
            CFlag::CY,
            ((self.ra as u16) < (val as u16 + cy as u16)) as u8,
        );

        let hc = (self.ra & 0xF) < ((val & 0xF) + cy);
        self.set_flag(CFlag::H, hc as u8);
        return res;
    }

    // cpu arithmetic instructions
    fn adc_ra(&mut self, val: u8) -> u8 {
        let rem = (self.ra & 0b1111) + (val & 0b1111) + self.get_flag(CFlag::CY);
        let res = self.ra as u16 + val as u16 + self.get_flag(CFlag::CY) as u16;

        self.ra = (res & 0b11111111) as u8;
        self.set_flag(CFlag::Z, (self.ra == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::CY, get_bit(res, 8));
        self.set_flag(CFlag::H, (rem & 0b10000) >> 4);
        return 1;
    }

    fn or_ra(&mut self, val: u8) -> u8 {
        self.ra |= val;
        self.set_flag(CFlag::Z, (self.ra == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, 0);
        return 1;
    }

    fn cp_ra(&mut self, val: u8) -> u8 {
        self.sub_ra(val);
        1
    }

    fn xor_ra(&mut self, val: u8) -> u8 {
        self.ra ^= val;
        self.set_flag(CFlag::Z, (self.ra == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, 0);
        1
    }
    fn rr(&mut self, val: u8) -> u8 {
        // rotate right through CY
        let b0 = val & 0x1;
        let res = (val >> 1) + (self.get_flag(CFlag::CY) << 7);

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::CY, b0);
        self.set_flag(CFlag::H, 0);
        return res;
    }
    fn rrc(&mut self, val: u8) -> u8 {
        let lsb = get_bit(val, 0);
        let res = (val >> 1) + (lsb << 7);

        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::CY, lsb);
        self.set_flag(CFlag::H, 0);
        return res;
    }
    fn and_ra(&mut self, val: u8) -> u8 {
        self.ra &= val;
        self.set_flag(CFlag::Z, (self.ra == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 1);
        self.set_flag(CFlag::CY, 0);
        1
    }

    fn ld_ra(&mut self, val: u8) -> u8 {
        self.ra = val;
        1
    }

    fn swap(&mut self, val: u8) -> u8 {
        let lower = val & 0b1111;
        let upper = val >> 4;
        let val = (lower << 4) + upper;
        self.set_flag(CFlag::Z, (val == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, 0);
        self.set_flag(CFlag::CY, 0);
        return val;
    }

    fn jr(&mut self, raddr: i8) {
        self.pc = self.pc.wrapping_add(raddr as u16);
    }

    fn inc(&mut self, val: u8) -> u8 {
        let res = val.wrapping_add(1);
        self.set_flag(CFlag::Z, (res == 0) as u8);
        self.set_flag(CFlag::S, 0);
        self.set_flag(CFlag::H, ((val & 0b1111) == 0b1111) as u8);
        return res;
    }
}

pub struct Runtime<'a> {
    pub memory: MMU<'a>,
    cpu: CpuRegisters,

    pub timer: Timer,
}

impl Memory for Runtime<'_> {
    fn get(&self, addr: u16) -> u8 {
        self.memory.get(addr)
    }

    fn set(&mut self, addr: u16, val: u8) -> () {
        self.memory.set(addr, val);
    }
    fn hwset(&mut self, addr: u16, val: u8) -> () {
        self.memory.set(addr, val);
    }
}

impl Runtime<'_> {
    // pub fn load<'a>(bootstrap: &'a Vec<u8>, rom: &'a Box<dyn Rom<'a>>) -> Runtime<'a> {
    pub fn load<'a>(bootstrap: &'a Vec<u8>, rom: &'a mut dyn Rom<'a>) -> Runtime<'a> {
        let rt = Runtime {
            cpu: CpuRegisters::new(),
            memory: MMU::new(&bootstrap, rom),
            timer: Timer::new(),
        };

        // https://b13rg.github.io/Gameboy-MBC-Analysis/#cart-1
        return rt;
    }

    pub fn noboot<'a>(bootstrap: &'a Vec<u8>, rom: &'a mut dyn Rom<'a>) -> Runtime<'a> {
        let mut rt = Runtime {
            cpu: CpuRegisters::atboot(),
            memory: MMU::new(&bootstrap, rom),
            timer: Timer::new(),
        };

        rt.memory.set(0xFF50, 1);

        // https://b13rg.github.io/Gameboy-MBC-Analysis/#cart-1
        return rt;
    }

    pub fn tick_timer(&mut self, ticks: u8) {
        self.timer.tick(&mut self.memory, ticks);
        self.memory.tick(ticks / 4);
    }
    fn next_opcode(&mut self) -> u8 {
        let opcode = self.get(self.cpu.pc);
        self.cpu.pc += 1;
        return opcode;
    }

    pub fn press_btn(&mut self, btn: HWInput) {
        self.memory.press(btn, true);
    }
    pub fn release_btn(&mut self, btn: HWInput) {
        self.memory.press(btn, false);
    }

    pub fn tick(&mut self) -> u8 {
        let interrupts = self.get(registers::IE) & self.get(registers::IF);
        if self.cpu.halt {
            if interrupts == 0 {
                // println!("wait for interrupt! {:b}", self.get(registers::IE));
                return 1;
            } else {
                self.cpu.halt = false;
            }
        }

        if self.cpu.ime && interrupts != 0 {
            self.cpu.ime = false;
            self.stack_push_u16(self.cpu.pc);

            let interrupt_flag = self.get(0xFF0F);
            // priority goes from lsb to msb
            if get_bit(interrupts, 0) == 1 {
                self.cpu.pc = 0x40;
                self.set(IF, set_bit(interrupt_flag, 0, false));
            } else if get_bit(interrupts, 1) == 1 {
                self.cpu.pc = 0x48;
                self.set(IF, set_bit(interrupt_flag, 1, false));
            } else if get_bit(interrupts, 2) == 1 {
                self.cpu.pc = 0x50;
                self.set(IF, set_bit(interrupt_flag, 2, false));
            } else if get_bit(interrupts, 3) == 1 {
                self.cpu.pc = 0x58;
                self.set(IF, set_bit(interrupt_flag, 3, false));
            } else if get_bit(interrupts, 4) == 1 {
                self.cpu.pc = 0x60;
                self.set(IF, set_bit(interrupt_flag, 4, false));
            }
        }

        if self.cpu.debug {
            println!(
                "{:?} PCMEM:{},{},{},{}",
                self.cpu,
                b64(self.get(self.cpu.pc + 0)),
                b64(self.get(self.cpu.pc + 1)),
                b64(self.get(self.cpu.pc + 2)),
                b64(self.get(self.cpu.pc + 3)),
            );
        }
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
            0x02 => {
                self.set(self.cpu.bc(), self.cpu.ra);
                2
            }
            0x03 => {
                self.cpu.set_bc(self.cpu.bc().wrapping_add(1));
                2
            }
            0x04 => {
                self.cpu.rb = self.cpu.inc(self.cpu.rb);
                1
            }
            0x05 => {
                // DEC B
                self.cpu.rb = self.cpu.dec(self.cpu.rb);
                1
            }
            0x06 => {
                self.cpu.rb = self.next_opcode();
                2
            }
            0x07 => {
                self.cpu.ra = self.cpu.rlc(self.cpu.ra);
                self.cpu.set_flag(CFlag::Z, 0);
                1
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
            0x09 => {
                let (cy, h, res) = add_u16(self.cpu.hl(), self.cpu.bc());

                self.cpu.set_hl(res);

                // self.cpu.set_flag(CFlag::Z, (res == 0) as u8);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::CY, cy);
                self.cpu.set_flag(CFlag::H, h);
                2
            }
            0x0A => {
                self.cpu.ra = self.get(self.cpu.bc());
                2
            }
            0x0B => {
                self.cpu.set_bc(self.cpu.bc().wrapping_sub(1));
                2
            }
            0x0C => {
                self.cpu.rc = self.cpu.inc(self.cpu.rc);
                1
            }
            0x0D => {
                self.cpu.rc = self.cpu.dec(self.cpu.rc);
                1
            }
            0x0E => {
                self.cpu.rc = self.next_opcode();
                2
            }
            0x0F => {
                self.cpu.ra = self.cpu.rrc(self.cpu.ra);
                self.cpu.set_flag(CFlag::Z, 0);
                1
            }
            0x10 => {
                println!("STOP instruction, enter CPU low power mode");
                1 // panic!("STOP!");
            }
            0x11 => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.cpu.set_de(join_u8(h, l));
                3
            }
            0x12 => {
                self.set(self.cpu.de(), self.cpu.ra);
                2
            }
            0x13 => {
                // INC DE
                self.cpu.set_de(self.cpu.de().wrapping_add(1));
                2
            }
            0x14 => {
                self.cpu.rd = self.cpu.inc(self.cpu.rd);
                1
            }
            0x15 => {
                self.cpu.rd = self.cpu.dec(self.cpu.rd);
                1
            }
            0x16 => {
                self.cpu.rd = self.next_opcode();
                2
            }
            0x17 => {
                // RLA
                let (cy, ra) = rl(self.cpu.get_flag(CFlag::CY), self.cpu.ra);
                self.cpu.ra = ra;
                self.cpu.set_flag(CFlag::CY, cy);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::H, 0);
                self.cpu.set_flag(CFlag::Z, 0);
                1
            }
            0x18 => {
                let raddr = self.next_opcode() as i8;
                self.cpu.jr(raddr);
                3
            }
            0x19 => {
                let (cy, h, res) = add_u16(self.cpu.hl(), self.cpu.de());

                self.cpu.set_hl(res);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::CY, cy as u8);
                self.cpu.set_flag(CFlag::H, h as u8);
                2
            }
            0x1A => {
                self.cpu.ra = self.get(self.cpu.de());
                2
            }
            0x1B => {
                self.cpu.set_de(self.cpu.de().wrapping_sub(1));
                2
            }
            0x1C => {
                self.cpu.re = self.cpu.inc(self.cpu.re);
                1
            }
            0x1D => {
                self.cpu.re = self.cpu.dec(self.cpu.re);
                1
            }
            0x1E => {
                self.cpu.re = self.next_opcode();
                2
            }
            0x1F => {
                self.cpu.ra = self.cpu.rr(self.cpu.ra);
                self.cpu.set_flag(CFlag::Z, 0);
                1
            }
            0x20 => {
                let addr = self.next_opcode() as i8;
                if self.cpu.get_flag(CFlag::Z) == 0 {
                    self.cpu.jr(addr);
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
                // LD (HL+), A
                self.set(self.cpu.hl(), self.cpu.ra);
                self.cpu.set_hl(self.cpu.hl().wrapping_add(1));
                2
            }
            0x23 => {
                // INC HL
                self.cpu.set_hl(self.cpu.hl().wrapping_add(1));
                2
            }
            0x24 => {
                self.cpu.rh = self.cpu.inc(self.cpu.rh);
                1
            }
            0x25 => {
                self.cpu.rh = self.cpu.dec(self.cpu.rh);
                1
            }
            0x26 => {
                self.cpu.rh = self.next_opcode();
                2
            }
            0x27 => {
                self.cpu.daa();
                1
            }
            0x28 => {
                let raddr = self.next_opcode() as i8;
                if self.cpu.get_flag(CFlag::Z) == 1 {
                    self.cpu.jr(raddr);
                    3
                } else {
                    2
                }
            }
            0x29 => {
                let (cy, h, res) = add_u16(self.cpu.hl(), self.cpu.hl());
                self.cpu.set_hl(res);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::CY, cy);
                self.cpu.set_flag(CFlag::H, h);
                2
            }
            0x2A => {
                self.cpu.ra = self.get(self.cpu.hl());
                self.cpu.set_hl(self.cpu.hl() + 1);
                2
            }
            0x2B => {
                self.cpu.set_hl(self.cpu.hl().wrapping_sub(1));
                2
            }
            0x2C => {
                self.cpu.rl = self.cpu.inc(self.cpu.rl);
                1
            }
            0x2D => {
                self.cpu.rl = self.cpu.dec(self.cpu.rl);
                1
            }
            0x2E => {
                self.cpu.rl = self.next_opcode();
                2
            }
            0x2F => {
                self.cpu.ra = !self.cpu.ra;
                self.cpu.set_flag(CFlag::S, 1);
                self.cpu.set_flag(CFlag::H, 1);
                1
            }
            0x30 => {
                let rel_addr = self.next_opcode();
                let cy = self.cpu.get_flag(CFlag::CY);
                if cy == 0 {
                    self.cpu.jr(rel_addr as i8);
                    3
                } else {
                    2
                }
            }
            0x31 => {
                // LD SP, d16
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.cpu.sp = join_u8(h, l);
                3
            }
            0x32 => {
                let hl = self.cpu.hl();
                self.set(hl, self.cpu.ra);
                self.cpu.set_hl(hl - 1);
                2
            }
            0x33 => {
                self.cpu.sp = self.cpu.sp.wrapping_add(1);
                2
            }
            0x34 => {
                let val = self.get(self.cpu.hl());
                let val = self.cpu.inc(val);
                self.set(self.cpu.hl(), val);
                3
            }
            0x35 => {
                let val = self.get(self.cpu.hl());
                let val = self.cpu.dec(val);
                self.set(self.cpu.hl(), val);
                3
            }
            0x36 => {
                let val = self.next_opcode();
                self.set(self.cpu.hl(), val);
                3
            }
            0x37 => {
                self.cpu.set_flag(CFlag::CY, 1);
                self.cpu.set_flag(CFlag::H, 0);
                self.cpu.set_flag(CFlag::S, 0);
                1
            }
            0x38 => {
                let raddr = self.next_opcode();
                if self.cpu.get_flag(CFlag::CY) == 1 {
                    self.cpu.jr(raddr as i8);
                    3
                } else {
                    2
                }
            }
            0x39 => {
                let (cy, hc, res) = add_u16(self.cpu.sp, self.cpu.hl());
                self.cpu.set_hl(res);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::H, hc);
                self.cpu.set_flag(CFlag::CY, cy);

                2
            }
            0x3A => {
                self.cpu.ra = self.get(self.cpu.hl());
                self.cpu.set_hl(self.cpu.hl().wrapping_sub(1));
                2
            }
            0x3B => {
                self.cpu.sp = self.cpu.sp.wrapping_sub(1);
                2
            }
            0x3C => {
                self.cpu.ra = self.cpu.inc(self.cpu.ra);
                1
            }
            0x3D => {
                self.cpu.ra = self.cpu.dec(self.cpu.ra);
                1
            }
            0x3E => {
                // LD A, d8
                self.cpu.ra = self.next_opcode();
                2
            }
            0x3F => {
                let cy = self.cpu.get_flag(CFlag::CY);
                self.cpu.set_flag(CFlag::CY, cy ^ 0x1);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::H, 0);
                1
            }
            0x40 => {
                self.cpu.rb = self.cpu.rb;
                1
            }
            0x41 => {
                self.cpu.rb = self.cpu.rc;
                1
            }
            0x42 => {
                self.cpu.rb = self.cpu.rd;
                1
            }
            0x43 => {
                self.cpu.rb = self.cpu.re;
                1
            }
            0x44 => {
                self.cpu.rb = self.cpu.rh;
                1
            }
            0x45 => {
                self.cpu.rb = self.cpu.rl;
                1
            }
            0x46 => {
                self.cpu.rb = self.get(self.cpu.hl());
                2
            }
            0x47 => {
                self.cpu.rb = self.cpu.ra;
                1
            }
            0x48 => {
                self.cpu.rc = self.cpu.rb;
                1
            }
            0x49 => {
                self.cpu.rc = self.cpu.rc;
                1
            }
            0x4A => {
                self.cpu.rc = self.cpu.rd;
                1
            }
            0x4B => {
                self.cpu.rc = self.cpu.re;
                1
            }
            0x4C => {
                self.cpu.rc = self.cpu.rh;
                1
            }
            0x4D => {
                self.cpu.rc = self.cpu.rl;
                1
            }
            0x4E => {
                self.cpu.rc = self.get(self.cpu.hl());
                2
            }
            0x4F => {
                self.cpu.rc = self.cpu.ra;
                1
            }
            0x50 => {
                self.cpu.rd = self.cpu.rb;
                1
            }
            0x51 => {
                self.cpu.rd = self.cpu.rc;
                1
            }
            0x52 => {
                self.cpu.rd = self.cpu.rd;
                1
            }
            0x53 => {
                self.cpu.rd = self.cpu.re;
                1
            }
            0x54 => {
                self.cpu.rd = self.cpu.rh;
                1
            }
            0x55 => {
                self.cpu.rd = self.cpu.rl;
                1
            }
            0x56 => {
                self.cpu.rd = self.get(self.cpu.hl());
                2
            }
            0x57 => {
                self.cpu.rd = self.cpu.ra;
                1
            }
            0x58 => {
                self.cpu.re = self.cpu.rb;
                1
            }
            0x59 => {
                self.cpu.re = self.cpu.rc;
                1
            }
            0x5A => {
                self.cpu.re = self.cpu.rd;
                1
            }
            0x5B => {
                self.cpu.re = self.cpu.re;
                1
            }
            0x5C => {
                self.cpu.re = self.cpu.rh;
                1
            }
            0x5D => {
                self.cpu.re = self.cpu.rl;
                1
            }
            0x5E => {
                self.cpu.re = self.get(self.cpu.hl());
                2
            }
            0x5F => {
                self.cpu.re = self.cpu.ra;
                1
            }
            0x60 => {
                self.cpu.rh = self.cpu.rb;
                1
            }
            0x61 => {
                self.cpu.rh = self.cpu.rc;
                1
            }
            0x62 => {
                self.cpu.rh = self.cpu.rd;
                1
            }
            0x63 => {
                self.cpu.rh = self.cpu.re;
                1
            }
            0x64 => {
                self.cpu.rh = self.cpu.rh;
                1
            }
            0x65 => {
                self.cpu.rh = self.cpu.rl;
                1
            }
            0x66 => {
                self.cpu.rh = self.get(self.cpu.hl());
                2
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
            0x6B => {
                self.cpu.rl = self.cpu.re;
                1
            }
            0x6C => {
                self.cpu.rl = self.cpu.rh;
                1
            }
            0x6D => {
                self.cpu.rl = self.cpu.rl;
                1
            }
            0x6E => {
                self.cpu.rl = self.get(self.cpu.hl());
                2
            }
            0x6F => {
                self.cpu.rl = self.cpu.ra;
                1
            }
            0x70 => {
                self.set(self.cpu.hl(), self.cpu.rb);
                2
            }
            0x71 => {
                self.set(self.cpu.hl(), self.cpu.rc);
                2
            }
            0x72 => {
                self.set(self.cpu.hl(), self.cpu.rd);
                2
            }
            0x73 => {
                self.set(self.cpu.hl(), self.cpu.re);
                2
            }
            0x74 => {
                self.set(self.cpu.hl(), self.cpu.rh);
                2
            }
            0x75 => {
                self.set(self.cpu.hl(), self.cpu.rl);
                2
            }
            0x76 => {
                self.cpu.halt = true;
                1
            }

            0x77 => {
                self.set(self.cpu.hl(), self.cpu.ra);
                2
            }

            0x78 => self.cpu.ld_ra(self.cpu.rb),
            0x79 => self.cpu.ld_ra(self.cpu.rc),
            0x7A => self.cpu.ld_ra(self.cpu.rd),
            0x7B => self.cpu.ld_ra(self.cpu.re),
            0x7C => self.cpu.ld_ra(self.cpu.rh),
            0x7D => self.cpu.ld_ra(self.cpu.rl),
            0x7E => self.cpu.ld_ra(self.get(self.cpu.hl())) * 2,
            0x7F => self.cpu.ld_ra(self.cpu.ra),

            0x80 => self.cpu.add_ra(self.cpu.rb),
            0x81 => self.cpu.add_ra(self.cpu.rc),
            0x82 => self.cpu.add_ra(self.cpu.rd),
            0x83 => self.cpu.add_ra(self.cpu.re),
            0x84 => self.cpu.add_ra(self.cpu.rh),
            0x85 => self.cpu.add_ra(self.cpu.rl),
            0x86 => self.cpu.add_ra(self.get(self.cpu.hl())) * 2,
            0x87 => self.cpu.add_ra(self.cpu.ra),

            0x88 => self.cpu.adc_ra(self.cpu.rb),
            0x89 => self.cpu.adc_ra(self.cpu.rc),
            0x8A => self.cpu.adc_ra(self.cpu.rd),
            0x8B => self.cpu.adc_ra(self.cpu.re),
            0x8C => self.cpu.adc_ra(self.cpu.rh),
            0x8D => self.cpu.adc_ra(self.cpu.rl),
            0x8E => self.cpu.adc_ra(self.get(self.cpu.hl())) * 2,
            0x8F => self.cpu.adc_ra(self.cpu.ra),

            0x90 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.rb);
                1
            }
            0x91 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.rc);
                1
            }
            0x92 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.rd);
                1
            }
            0x93 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.re);
                1
            }
            0x94 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.rh);
                1
            }
            0x95 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.rl);
                1
            }
            0x96 => {
                self.cpu.ra = self.cpu.sub_ra(self.get(self.cpu.hl()));
                2
            }
            0x97 => {
                self.cpu.ra = self.cpu.sub_ra(self.cpu.ra);
                1
            }

            0x98 => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.rb);
                1
            }
            0x99 => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.rc);
                1
            }
            0x9A => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.rd);
                1
            }
            0x9B => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.re);
                1
            }
            0x9C => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.rh);
                1
            }
            0x9D => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.rl);
                1
            }
            0x9E => {
                self.cpu.ra = self.cpu.sbc_ra(self.get(self.cpu.hl()));
                2
            }
            0x9F => {
                self.cpu.ra = self.cpu.sbc_ra(self.cpu.ra);
                1
            }

            0xA0 => self.cpu.and_ra(self.cpu.rb),
            0xA1 => self.cpu.and_ra(self.cpu.rc),
            0xA2 => self.cpu.and_ra(self.cpu.rd),
            0xA3 => self.cpu.and_ra(self.cpu.re),
            0xA4 => self.cpu.and_ra(self.cpu.rh),
            0xA5 => self.cpu.and_ra(self.cpu.rl),
            0xA6 => self.cpu.and_ra(self.get(self.cpu.hl())) * 2,
            0xA7 => self.cpu.and_ra(self.cpu.ra),

            0xA8 => self.cpu.xor_ra(self.cpu.rb),
            0xA9 => self.cpu.xor_ra(self.cpu.rc),
            0xAA => self.cpu.xor_ra(self.cpu.rd),
            0xAB => self.cpu.xor_ra(self.cpu.re),
            0xAC => self.cpu.xor_ra(self.cpu.rh),
            0xAD => self.cpu.xor_ra(self.cpu.rl),
            0xAE => self.cpu.xor_ra(self.get(self.cpu.hl())) * 2,
            0xAF => self.cpu.xor_ra(self.cpu.ra),

            0xB0 => self.cpu.or_ra(self.cpu.rb),
            0xB1 => self.cpu.or_ra(self.cpu.rc),
            0xB2 => self.cpu.or_ra(self.cpu.rd),
            0xB3 => self.cpu.or_ra(self.cpu.re),
            0xB4 => self.cpu.or_ra(self.cpu.rh),
            0xB5 => self.cpu.or_ra(self.cpu.rl),
            0xB6 => self.cpu.or_ra(self.get(self.cpu.hl())) * 2,
            0xB7 => self.cpu.or_ra(self.cpu.ra),

            0xB8 => self.cpu.cp_ra(self.cpu.rb),
            0xB9 => self.cpu.cp_ra(self.cpu.rc),
            0xBA => self.cpu.cp_ra(self.cpu.rd),
            0xBB => self.cpu.cp_ra(self.cpu.re),
            0xBC => self.cpu.cp_ra(self.cpu.rh),
            0xBD => self.cpu.cp_ra(self.cpu.rl),
            0xBE => self.cpu.cp_ra(self.get(self.cpu.hl())) * 2,
            0xBF => self.cpu.cp_ra(self.cpu.ra),

            0xC0 => {
                if self.cpu.get_flag(CFlag::Z) == 0 {
                    self.cpu.pc = self.stack_pop_u16();
                    5
                } else {
                    2
                }
            }
            0xC1 => {
                let val = self.stack_pop_u16();
                self.cpu.set_bc(val);
                3
            }
            0xC2 => {
                let l = self.next_opcode();
                let h = self.next_opcode();

                if self.cpu.get_flag(CFlag::Z) == 0 {
                    self.cpu.pc = join_u8(h, l);
                    4
                } else {
                    3
                }
            }
            0xC3 => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.cpu.pc = join_u8(h, l);
                4
            }
            0xC4 => {
                let l = self.next_opcode();
                let h = self.next_opcode();

                if self.cpu.get_flag(CFlag::Z) == 0 {
                    self.stack_push_u16(self.cpu.pc);
                    self.cpu.pc = join_u8(h, l);
                    6
                } else {
                    3
                }
            }
            0xC5 => {
                // PUSH BC
                self.stack_push_u16(self.cpu.bc());
                4
            }
            0xC6 => {
                let imm = self.next_opcode();
                self.cpu.add_ra(imm);
                2
            }
            0xC7 => {
                // RST 0
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x00;
                4
            }
            0xC8 => {
                if self.cpu.get_flag(CFlag::Z) == 1 {
                    self.cpu.pc = self.stack_pop_u16();
                    5
                } else {
                    2
                }
            }
            0xC9 => {
                self.cpu.pc = self.stack_pop_u16();
                4
            }
            0xCA => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                if self.cpu.get_flag(CFlag::Z) == 1 {
                    self.cpu.pc = join_u8(h, l);
                    4
                } else {
                    3
                }
            }
            0xCB => {
                let opnext = self.next_opcode();
                self.eval_cb(opnext)
            }
            0xCC => {
                let l = self.next_opcode();
                let h = self.next_opcode();

                if self.cpu.get_flag(CFlag::Z) == 1 {
                    self.stack_push_u16(self.cpu.pc);
                    self.cpu.pc = join_u8(h, l);
                    6
                } else {
                    3
                }
            }
            0xCD => {
                // CALL a16
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.stack_push_u16(self.cpu.pc); // save PC to the stack
                self.cpu.pc = join_u8(h, l);
                6
            }
            0xCE => {
                let imm = self.next_opcode();
                self.cpu.adc_ra(imm);
                2
            }
            0xCF => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x08;
                4
            }
            0xD0 => {
                if self.cpu.get_flag(CFlag::CY) == 0 {
                    self.cpu.pc = self.stack_pop_u16();
                    5
                } else {
                    2
                }
            }
            0xD1 => {
                let de = self.stack_pop_u16();
                self.cpu.set_de(de);
                3
            }
            0xD2 => {
                let l = self.next_opcode();
                let h = self.next_opcode();

                if self.cpu.get_flag(CFlag::CY) == 0 {
                    self.cpu.pc = join_u8(h, l);
                    4
                } else {
                    3
                }
            }
            0xD4 => {
                let l = self.next_opcode();
                let h = self.next_opcode();

                if self.cpu.get_flag(CFlag::CY) == 0 {
                    self.stack_push_u16(self.cpu.pc);
                    self.cpu.pc = join_u8(h, l);
                    6
                } else {
                    3
                }
            }

            0xD5 => {
                self.stack_push_u16(self.cpu.de());
                4
            }
            0xD6 => {
                let op = self.next_opcode();
                self.cpu.ra = self.cpu.sub_ra(op);
                2
            }
            0xD7 => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x10;
                4
            }
            0xD8 => {
                if self.cpu.get_flag(CFlag::CY) == 0x1 {
                    self.cpu.pc = self.stack_pop_u16();
                    5
                } else {
                    2
                }
            }
            0xD9 => {
                self.cpu.pc = self.stack_pop_u16();
                self.cpu.ime = true;
                4
            }
            0xDA => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                if self.cpu.get_flag(CFlag::CY) == 0x1 {
                    self.cpu.pc = join_u8(h, l);
                    4
                } else {
                    3
                }
            }
            0xDC => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                if self.cpu.get_flag(CFlag::CY) == 0x1 {
                    self.stack_push_u16(self.cpu.pc);
                    self.cpu.pc = join_u8(h, l);
                    6
                } else {
                    3
                }
            }
            0xDE => {
                let val = self.next_opcode();
                self.cpu.ra = self.cpu.sbc_ra(val);
                2
            }
            0xDF => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x18;
                4
            }
            0xE0 => {
                let addr = 0xFF00 + self.next_opcode() as u16;
                self.set(addr, self.cpu.ra);
                3
            }
            0xE1 => {
                let hl = self.stack_pop_u16();
                self.cpu.set_hl(hl);
                3
            }
            0xE2 => {
                let addr = 0xFF00 + self.cpu.rc as u16;
                self.set(addr, self.cpu.ra);
                2
            }
            0xE5 => {
                self.stack_push_u16(self.cpu.hl());
                4
            }

            0xE6 => {
                let op = self.next_opcode();
                self.cpu.and_ra(op);
                2
            }
            0xE7 => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x20;
                4
            }
            0xE8 => {
                let op = self.next_opcode() as i8;
                self.cpu.sp = self.cpu.add_u16_i8(self.cpu.sp, op);
                self.cpu.set_flag(CFlag::Z, 0);
                4
            }

            0xE9 => {
                self.cpu.pc = self.cpu.hl();
                1
            }
            0xEA => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                let addr = join_u8(h, l);
                self.set(addr, self.cpu.ra);
                4
            }
            0xEE => {
                let op = self.next_opcode();
                self.cpu.xor_ra(op);
                2
            }
            0xEF => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x28;
                4
            }
            0xF0 => {
                let addr = 0xFF00 + self.next_opcode() as u16;
                self.cpu.ra = self.get(addr);
                3
            }
            0xF1 => {
                let val = self.stack_pop_u16();
                self.cpu.set_af(val);
                3
            }
            0xF2 => {
                let addr = 0xFF00 + self.cpu.rc as u16;
                self.cpu.ra = self.get(addr);
                2
            }
            0xF3 => {
                // DI
                self.cpu.ime = false;
                1
            }
            0xF5 => {
                let af = self.cpu.af();
                self.stack_push_u16(af);
                4
            }
            0xF6 => {
                self.cpu.ra |= self.next_opcode();
                self.cpu.set_flag(CFlag::Z, (self.cpu.ra == 0) as u8);
                self.cpu.set_flag(CFlag::S, 0);
                self.cpu.set_flag(CFlag::H, 0);
                self.cpu.set_flag(CFlag::CY, 0);
                2
            }
            0xF7 => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x30;
                4
            }
            0xF8 => {
                let op = self.next_opcode() as i8;
                let res = self.cpu.add_u16_i8(self.cpu.sp, op);
                self.cpu.set_hl(res);
                self.cpu.set_flag(CFlag::Z, 0);
                3
            }
            0xF9 => {
                self.cpu.sp = self.cpu.hl();
                2
            }
            0xFA => {
                let l = self.next_opcode();
                let h = self.next_opcode();
                self.cpu.ra = self.get(join_u8(h, l));
                4
            }
            0xFB => {
                // EI
                self.cpu.ime = true;
                1
            }
            0xFE => {
                let imm = self.next_opcode();
                self.cpu.sub_ra(imm);
                2
            }
            0xFF => {
                self.stack_push_u16(self.cpu.pc);
                self.cpu.pc = 0x38;
                4
            }
            _ => {
                panic!("ERROR: Opcode 0x{} not implemented!", b64(opcode));
            }
        };
    }

    fn eval_cb(&mut self, opcode: u8) -> u8 {
        return match opcode {
            0x00 => {
                self.cpu.rb = self.cpu.rlc(self.cpu.rb);
                2
            }
            0x01 => {
                self.cpu.rc = self.cpu.rlc(self.cpu.rc);
                2
            }
            0x02 => {
                self.cpu.rd = self.cpu.rlc(self.cpu.rd);
                2
            }
            0x03 => {
                self.cpu.re = self.cpu.rlc(self.cpu.re);
                2
            }
            0x04 => {
                self.cpu.rh = self.cpu.rlc(self.cpu.rh);
                2
            }
            0x05 => {
                self.cpu.rl = self.cpu.rlc(self.cpu.rl);
                2
            }
            0x06 => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.rlc(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x07 => {
                self.cpu.ra = self.cpu.rlc(self.cpu.ra);
                2
            }

            0x08 => {
                self.cpu.rb = self.cpu.rrc(self.cpu.rb);
                2
            }
            0x09 => {
                self.cpu.rc = self.cpu.rrc(self.cpu.rc);
                2
            }
            0x0A => {
                self.cpu.rd = self.cpu.rrc(self.cpu.rd);
                2
            }
            0x0B => {
                self.cpu.re = self.cpu.rrc(self.cpu.re);
                2
            }
            0x0C => {
                self.cpu.rh = self.cpu.rrc(self.cpu.rh);
                2
            }
            0x0D => {
                self.cpu.rl = self.cpu.rrc(self.cpu.rl);
                2
            }
            0x0E => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.rrc(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x0F => {
                self.cpu.ra = self.cpu.rrc(self.cpu.ra);
                2
            }

            0x10 => {
                self.cpu.rb = self.cpu.rl(self.cpu.rb);
                2
            }
            0x11 => {
                self.cpu.rc = self.cpu.rl(self.cpu.rc);
                2
            }
            0x12 => {
                self.cpu.rd = self.cpu.rl(self.cpu.rd);
                2
            }
            0x13 => {
                self.cpu.re = self.cpu.rl(self.cpu.re);
                2
            }
            0x14 => {
                self.cpu.rh = self.cpu.rl(self.cpu.rh);
                2
            }
            0x15 => {
                self.cpu.rl = self.cpu.rl(self.cpu.rl);
                2
            }
            0x16 => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.rl(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x17 => {
                self.cpu.ra = self.cpu.rl(self.cpu.ra);
                2
            }

            0x18 => {
                self.cpu.rb = self.cpu.rr(self.cpu.rb);
                2
            }
            0x19 => {
                self.cpu.rc = self.cpu.rr(self.cpu.rc);
                2
            }
            0x1A => {
                self.cpu.rd = self.cpu.rr(self.cpu.rd);
                2
            }
            0x1B => {
                self.cpu.re = self.cpu.rr(self.cpu.re);
                2
            }
            0x1C => {
                self.cpu.rh = self.cpu.rr(self.cpu.rh);
                2
            }
            0x1D => {
                self.cpu.rl = self.cpu.rr(self.cpu.rl);
                2
            }
            0x1E => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.rr(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x1F => {
                self.cpu.ra = self.cpu.rr(self.cpu.ra);
                2
            }

            0x20 => {
                self.cpu.rb = self.cpu.sla(self.cpu.rb);
                2
            }
            0x21 => {
                self.cpu.rc = self.cpu.sla(self.cpu.rc);
                2
            }
            0x22 => {
                self.cpu.rd = self.cpu.sla(self.cpu.rd);
                2
            }
            0x23 => {
                self.cpu.re = self.cpu.sla(self.cpu.re);
                2
            }
            0x24 => {
                self.cpu.rh = self.cpu.sla(self.cpu.rh);
                2
            }
            0x25 => {
                self.cpu.rl = self.cpu.sla(self.cpu.rl);
                2
            }
            0x26 => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.sla(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x27 => {
                self.cpu.ra = self.cpu.sla(self.cpu.ra);
                2
            }

            0x28 => {
                self.cpu.rb = self.cpu.sra(self.cpu.rb);
                2
            }
            0x29 => {
                self.cpu.rc = self.cpu.sra(self.cpu.rc);
                2
            }
            0x2A => {
                self.cpu.rd = self.cpu.sra(self.cpu.rd);
                2
            }
            0x2B => {
                self.cpu.re = self.cpu.sra(self.cpu.re);
                2
            }
            0x2C => {
                self.cpu.rh = self.cpu.sra(self.cpu.rh);
                2
            }
            0x2D => {
                self.cpu.rl = self.cpu.sra(self.cpu.rl);
                2
            }
            0x2E => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.sra(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x2F => {
                self.cpu.ra = self.cpu.sra(self.cpu.ra);
                2
            }

            0x30 => {
                self.cpu.rb = self.cpu.swap(self.cpu.rb);
                2
            }
            0x31 => {
                self.cpu.rc = self.cpu.swap(self.cpu.rc);
                2
            }
            0x32 => {
                self.cpu.rd = self.cpu.swap(self.cpu.rd);
                2
            }
            0x33 => {
                self.cpu.re = self.cpu.swap(self.cpu.re);
                2
            }
            0x34 => {
                self.cpu.rh = self.cpu.swap(self.cpu.rh);
                2
            }
            0x35 => {
                self.cpu.rl = self.cpu.swap(self.cpu.rl);
                2
            }
            0x36 => {
                let val = self.cpu.swap(self.get(self.cpu.hl()));
                self.set(self.cpu.hl(), val);
                4
            }
            0x37 => {
                self.cpu.ra = self.cpu.swap(self.cpu.ra);
                2
            }

            0x38 => {
                self.cpu.rb = self.cpu.srl(self.cpu.rb);
                2
            }
            0x39 => {
                self.cpu.rc = self.cpu.srl(self.cpu.rc);
                2
            }
            0x3A => {
                self.cpu.rd = self.cpu.srl(self.cpu.rd);
                2
            }
            0x3B => {
                self.cpu.re = self.cpu.srl(self.cpu.re);
                2
            }
            0x3C => {
                self.cpu.rh = self.cpu.srl(self.cpu.rh);
                2
            }
            0x3D => {
                self.cpu.rl = self.cpu.srl(self.cpu.rl);
                2
            }
            0x3E => {
                let hl = self.get(self.cpu.hl());
                let hl = self.cpu.srl(hl);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x3F => {
                self.cpu.ra = self.cpu.srl(self.cpu.ra);
                2
            }

            0x40 => {
                self.cpu.bit(self.cpu.rb, 0);
                2
            }
            0x41 => {
                self.cpu.bit(self.cpu.rc, 0);
                2
            }
            0x42 => {
                self.cpu.bit(self.cpu.rd, 0);
                2
            }
            0x43 => {
                self.cpu.bit(self.cpu.re, 0);
                2
            }
            0x44 => {
                self.cpu.bit(self.cpu.rh, 0);
                2
            }
            0x45 => {
                self.cpu.bit(self.cpu.rl, 0);
                2
            }
            0x46 => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 0);
                3
            }
            0x47 => {
                self.cpu.bit(self.cpu.ra, 0);
                2
            }

            0x48 => {
                self.cpu.bit(self.cpu.rb, 1);
                2
            }
            0x49 => {
                self.cpu.bit(self.cpu.rc, 1);
                2
            }
            0x4A => {
                self.cpu.bit(self.cpu.rd, 1);
                2
            }
            0x4B => {
                self.cpu.bit(self.cpu.re, 1);
                2
            }
            0x4C => {
                self.cpu.bit(self.cpu.rh, 1);
                2
            }
            0x4D => {
                self.cpu.bit(self.cpu.rl, 1);
                2
            }
            0x4E => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 1);
                3
            }
            0x4F => {
                self.cpu.bit(self.cpu.ra, 1);
                2
            }

            0x50 => {
                self.cpu.bit(self.cpu.rb, 2);
                2
            }
            0x51 => {
                self.cpu.bit(self.cpu.rc, 2);
                2
            }
            0x52 => {
                self.cpu.bit(self.cpu.rd, 2);
                2
            }
            0x53 => {
                self.cpu.bit(self.cpu.re, 2);
                2
            }
            0x54 => {
                self.cpu.bit(self.cpu.rh, 2);
                2
            }
            0x55 => {
                self.cpu.bit(self.cpu.rl, 2);
                2
            }
            0x56 => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 2);
                3
            }
            0x57 => {
                self.cpu.bit(self.cpu.ra, 2);
                2
            }

            0x58 => {
                self.cpu.bit(self.cpu.rb, 3);
                2
            }
            0x59 => {
                self.cpu.bit(self.cpu.rc, 3);
                2
            }
            0x5A => {
                self.cpu.bit(self.cpu.rd, 3);
                2
            }
            0x5B => {
                self.cpu.bit(self.cpu.re, 3);
                2
            }
            0x5C => {
                self.cpu.bit(self.cpu.rh, 3);
                2
            }
            0x5D => {
                self.cpu.bit(self.cpu.rl, 3);
                2
            }
            0x5E => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 3);
                3
            }
            0x5F => {
                self.cpu.bit(self.cpu.ra, 3);
                2
            }

            0x60 => {
                self.cpu.bit(self.cpu.rb, 4);
                2
            }
            0x61 => {
                self.cpu.bit(self.cpu.rc, 4);
                2
            }
            0x62 => {
                self.cpu.bit(self.cpu.rd, 4);
                2
            }
            0x63 => {
                self.cpu.bit(self.cpu.re, 4);
                2
            }
            0x64 => {
                self.cpu.bit(self.cpu.rh, 4);
                2
            }
            0x65 => {
                self.cpu.bit(self.cpu.rl, 4);
                2
            }
            0x66 => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 4);
                3
            }
            0x67 => {
                self.cpu.bit(self.cpu.ra, 4);
                2
            }

            0x68 => {
                self.cpu.bit(self.cpu.rb, 5);
                2
            }
            0x69 => {
                self.cpu.bit(self.cpu.rc, 5);
                2
            }
            0x6A => {
                self.cpu.bit(self.cpu.rd, 5);
                2
            }
            0x6B => {
                self.cpu.bit(self.cpu.re, 5);
                2
            }
            0x6C => {
                self.cpu.bit(self.cpu.rh, 5);
                2
            }
            0x6D => {
                self.cpu.bit(self.cpu.rl, 5);
                2
            }
            0x6E => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 5);
                3
            }
            0x6F => {
                self.cpu.bit(self.cpu.ra, 5);
                2
            }

            0x70 => {
                self.cpu.bit(self.cpu.rb, 6);
                2
            }
            0x71 => {
                self.cpu.bit(self.cpu.rc, 6);
                2
            }
            0x72 => {
                self.cpu.bit(self.cpu.rd, 6);
                2
            }
            0x73 => {
                self.cpu.bit(self.cpu.re, 6);
                2
            }
            0x74 => {
                self.cpu.bit(self.cpu.rh, 6);
                2
            }
            0x75 => {
                self.cpu.bit(self.cpu.rl, 6);
                2
            }
            0x76 => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 6);
                3
            }
            0x77 => {
                self.cpu.bit(self.cpu.ra, 6);
                2
            }

            0x78 => {
                self.cpu.bit(self.cpu.rb, 7);
                2
            }
            0x79 => {
                self.cpu.bit(self.cpu.rc, 7);
                2
            }
            0x7A => {
                self.cpu.bit(self.cpu.rd, 7);
                2
            }
            0x7B => {
                self.cpu.bit(self.cpu.re, 7);
                2
            }
            0x7C => {
                self.cpu.bit(self.cpu.rh, 7);
                2
            }
            0x7D => {
                self.cpu.bit(self.cpu.rl, 7);
                2
            }
            0x7E => {
                let hl = self.get(self.cpu.hl());
                self.cpu.bit(hl, 7);
                3
            }
            0x7F => {
                self.cpu.bit(self.cpu.ra, 7);
                2
            }

            0x80 => res(&mut self.cpu.rb, 0),
            0x81 => res(&mut self.cpu.rc, 0),
            0x82 => res(&mut self.cpu.rd, 0),
            0x83 => res(&mut self.cpu.re, 0),
            0x84 => res(&mut self.cpu.rh, 0),
            0x85 => res(&mut self.cpu.rl, 0),
            0x86 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 0, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x87 => res(&mut self.cpu.ra, 0),

            0x88 => res(&mut self.cpu.rb, 1),
            0x89 => res(&mut self.cpu.rc, 1),
            0x8A => res(&mut self.cpu.rd, 1),
            0x8B => res(&mut self.cpu.re, 1),
            0x8C => res(&mut self.cpu.rh, 1),
            0x8D => res(&mut self.cpu.rl, 1),
            0x8E => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 1, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x8F => res(&mut self.cpu.ra, 1),

            0x90 => res(&mut self.cpu.rb, 2),
            0x91 => res(&mut self.cpu.rc, 2),
            0x92 => res(&mut self.cpu.rd, 2),
            0x93 => res(&mut self.cpu.re, 2),
            0x94 => res(&mut self.cpu.rh, 2),
            0x95 => res(&mut self.cpu.rl, 2),
            0x96 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 2, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x97 => res(&mut self.cpu.ra, 2),

            0x98 => res(&mut self.cpu.rb, 3),
            0x99 => res(&mut self.cpu.rc, 3),
            0x9A => res(&mut self.cpu.rd, 3),
            0x9B => res(&mut self.cpu.re, 3),
            0x9C => res(&mut self.cpu.rh, 3),
            0x9D => res(&mut self.cpu.rl, 3),
            0x9E => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 3, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0x9F => res(&mut self.cpu.ra, 3),

            0xA0 => res(&mut self.cpu.rb, 4),
            0xA1 => res(&mut self.cpu.rc, 4),
            0xA2 => res(&mut self.cpu.rd, 4),
            0xA3 => res(&mut self.cpu.re, 4),
            0xA4 => res(&mut self.cpu.rh, 4),
            0xA5 => res(&mut self.cpu.rl, 4),
            0xA6 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 4, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xA7 => res(&mut self.cpu.ra, 4),

            0xA8 => res(&mut self.cpu.rb, 5),
            0xA9 => res(&mut self.cpu.rc, 5),
            0xAA => res(&mut self.cpu.rd, 5),
            0xAB => res(&mut self.cpu.re, 5),
            0xAC => res(&mut self.cpu.rh, 5),
            0xAD => res(&mut self.cpu.rl, 5),
            0xAE => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 5, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xAF => res(&mut self.cpu.ra, 5),

            0xB0 => res(&mut self.cpu.rb, 6),
            0xB1 => res(&mut self.cpu.rc, 6),
            0xB2 => res(&mut self.cpu.rd, 6),
            0xB3 => res(&mut self.cpu.re, 6),
            0xB4 => res(&mut self.cpu.rh, 6),
            0xB5 => res(&mut self.cpu.rl, 6),
            0xB6 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 6, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xB7 => res(&mut self.cpu.ra, 6),

            0xB8 => res(&mut self.cpu.rb, 7),
            0xB9 => res(&mut self.cpu.rc, 7),
            0xBA => res(&mut self.cpu.rd, 7),
            0xBB => res(&mut self.cpu.re, 7),
            0xBC => res(&mut self.cpu.rh, 7),
            0xBD => res(&mut self.cpu.rl, 7),
            0xBE => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 7, false);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xBF => res(&mut self.cpu.ra, 7),

            0xC0 => set(&mut self.cpu.rb, 0),
            0xC1 => set(&mut self.cpu.rc, 0),
            0xC2 => set(&mut self.cpu.rd, 0),
            0xC3 => set(&mut self.cpu.re, 0),
            0xC4 => set(&mut self.cpu.rh, 0),
            0xC5 => set(&mut self.cpu.rl, 0),
            0xC6 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 0, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xC7 => set(&mut self.cpu.ra, 0),

            0xC8 => set(&mut self.cpu.rb, 1),
            0xC9 => set(&mut self.cpu.rc, 1),
            0xCA => set(&mut self.cpu.rd, 1),
            0xCB => set(&mut self.cpu.re, 1),
            0xCC => set(&mut self.cpu.rh, 1),
            0xCD => set(&mut self.cpu.rl, 1),
            0xCE => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 1, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xCF => set(&mut self.cpu.ra, 1),

            0xD0 => set(&mut self.cpu.rb, 2),
            0xD1 => set(&mut self.cpu.rc, 2),
            0xD2 => set(&mut self.cpu.rd, 2),
            0xD3 => set(&mut self.cpu.re, 2),
            0xD4 => set(&mut self.cpu.rh, 2),
            0xD5 => set(&mut self.cpu.rl, 2),
            0xD6 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 2, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xD7 => set(&mut self.cpu.ra, 2),

            0xD8 => set(&mut self.cpu.rb, 3),
            0xD9 => set(&mut self.cpu.rc, 3),
            0xDA => set(&mut self.cpu.rd, 3),
            0xDB => set(&mut self.cpu.re, 3),
            0xDC => set(&mut self.cpu.rh, 3),
            0xDD => set(&mut self.cpu.rl, 3),
            0xDE => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 3, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xDF => set(&mut self.cpu.ra, 3),

            0xE0 => set(&mut self.cpu.rb, 4),
            0xE1 => set(&mut self.cpu.rc, 4),
            0xE2 => set(&mut self.cpu.rd, 4),
            0xE3 => set(&mut self.cpu.re, 4),
            0xE4 => set(&mut self.cpu.rh, 4),
            0xE5 => set(&mut self.cpu.rl, 4),
            0xE6 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 4, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xE7 => set(&mut self.cpu.ra, 4),

            0xE8 => set(&mut self.cpu.rb, 5),
            0xE9 => set(&mut self.cpu.rc, 5),
            0xEA => set(&mut self.cpu.rd, 5),
            0xEB => set(&mut self.cpu.re, 5),
            0xEC => set(&mut self.cpu.rh, 5),
            0xED => set(&mut self.cpu.rl, 5),
            0xEE => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 5, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xEF => set(&mut self.cpu.ra, 5),

            0xF0 => set(&mut self.cpu.rb, 6),
            0xF1 => set(&mut self.cpu.rc, 6),
            0xF2 => set(&mut self.cpu.rd, 6),
            0xF3 => set(&mut self.cpu.re, 6),
            0xF4 => set(&mut self.cpu.rh, 6),
            0xF5 => set(&mut self.cpu.rl, 6),
            0xF6 => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 6, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xF7 => set(&mut self.cpu.ra, 6),

            0xF8 => set(&mut self.cpu.rb, 7),
            0xF9 => set(&mut self.cpu.rc, 7),
            0xFA => set(&mut self.cpu.rd, 7),
            0xFB => set(&mut self.cpu.re, 7),
            0xFC => set(&mut self.cpu.rh, 7),
            0xFD => set(&mut self.cpu.rl, 7),
            0xFE => {
                let hl = self.get(self.cpu.hl());
                let hl = set_bit(hl, 7, true);
                self.set(self.cpu.hl(), hl);
                4
            }
            0xFF => set(&mut self.cpu.ra, 7),

            _ => {
                panic!("ERROR: Opcode CB{} not implemented", b64(opcode));
            }
        };
    }

    fn boot_rom_disabled(&self) -> bool {
        return self.get(0xFF50) == 1;
    }
    // RAM

    // STACK
    fn stack_push_u16(&mut self, value: u16) {
        // println!("Push: {}", value);
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
        let val = join_u8(h, l);
        // println!("Pop: {}", val);
        return val;
    }
}

fn add_u16(a: u16, b: u16) -> (u8, u8, u16) {
    let res = a as u32 + b as u32;
    let cy = (res & (1 << 16)) >> 16;

    // higher nibble half carry
    let hc = (((a & 0xFFF) + (b & 0xFFF)) & 0x1000) == 0x1000;
    return (cy as u8, hc as u8, res as u16);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_returns_b64_numbers() {
        assert_eq!(b64(0xFFu16), "00FF".to_string());
        assert_eq!(b64(0x3Au16), "003A".to_string());
    }

    #[test]
    fn test_add_ra() {
        let mut cpu = CpuRegisters::new();
        cpu.add_ra(1);
        assert_eq!(cpu.ra, 1);
    }

    #[test]
    fn test_add_ra_with_starting_value() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 6;
        cpu.add_ra(1);
        assert_eq!(cpu.ra, 7);
    }

    #[test]
    fn test_add_ra_wraps() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 0b11111111;
        cpu.add_ra(1);
        assert_eq!(cpu.ra, 0);
    }

    #[test]
    fn test_add_ra_sets_zero_flag() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 0b11111111;
        cpu.add_ra(1);

        assert_eq!(0x1, cpu.get_flag(CFlag::Z));
    }

    #[test]
    fn test_add_ra_clears_zero_flag() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 0b11111110;
        cpu.add_ra(1);

        assert_eq!(0x0, cpu.get_flag(CFlag::Z));
    }

    #[test]
    fn test_sets_carry() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 0b11111111;
        cpu.add_ra(1);

        assert_eq!(0x1, cpu.get_flag(CFlag::CY));
    }

    #[test]
    fn test_sets_half_carry() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 0b1111;
        cpu.add_ra(1);
        assert_eq!(0x1, cpu.get_flag(CFlag::H));
    }

    #[test]
    fn test_clears_half_carry() {
        let mut cpu = CpuRegisters::new();
        cpu.ra = 0b111;
        cpu.add_ra(1);
        assert_eq!(0x0, cpu.get_flag(CFlag::H));
    }

    #[test]
    fn test_res_changes_inplace() {
        let mut val = 0b111;
        res(&mut val, 1);
        assert_eq!(val, 0b101);
    }

    #[test]
    fn test_jr_jumps_ahead() {
        let mut cpu = CpuRegisters::new();
        cpu.jr(10);
        assert_eq!(cpu.pc, 10);
    }

    #[test]
    fn test_jr_jumps_behind() {
        let mut cpu = CpuRegisters::new();
        cpu.pc = 10;
        cpu.jr(-2);
        assert_eq!(cpu.pc, 8);
    }

    #[test]
    fn test_add_u16_has_carry() {
        let max = ((1 << 16) - 1) as u16;
        let (cy, h, res) = add_u16(max, max);

        assert_eq!(cy, 1);
    }

    #[test]
    fn test_add_u16_wraps_result() {
        let max = ((1 << 16) - 1) as u16;
        let (cy, h, res) = add_u16(max, max);

        assert_eq!(res, (max as u32 * 2) as u16);
    }

    #[test]
    fn test_add_u16_half_carry() {
        let (_, h, _) = add_u16(0x4C00, 0x4C00);
        assert_eq!(h, 1);
    }

    #[test]
    fn test_inc_does_not_subtract() {
        let mut cpu = CpuRegisters::new();
        cpu.inc(0xE);

        assert_eq!(cpu.rf, 0);
    }

    #[test]
    fn test_push_pop_af() {
        let mut cpu = CpuRegisters::new();
        cpu.set_af(0x1234);

        assert_eq!(cpu.af(), 0x1234 & 0xFFF0);
        assert_eq!(cpu.ra, 0x12);
        assert_eq!(cpu.rf, 0x34 & 0xF0);
    }
}
