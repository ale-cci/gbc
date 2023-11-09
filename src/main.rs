use std::fs;
use std::io::Read;
// https://retrocomputing.stackexchange.com/questions/11732/how-does-the-gameboys-memory-bank-switching-work
// https://realboyemulator.wordpress.com/2013/01/03/a-look-at-the-game-boy-bootstrap-let-the-fun-begin/
// https://gekkio.fi/files/gb-docs/gbctr.pdf

#[derive(Debug)]
struct CpuRegisters {
    ra: u8, rf: u8,
    rb: u8, rc: u8,
    rd: u8, re: u8,
    rh: u8, rl: u8,
    sp: u16,
    pc: u16,

}
struct Runtime<'a> {
    cpu: CpuRegisters,
    rom: &'a Vec<u8>,
    bootstrap: &'a Vec<u8>,
}

fn b_repr(byte: u8) -> char {
    if byte <= 9 {
        return (byte + '0' as u8) as char;
    }
    return (byte - 10 + 'A' as u8) as char;
}

fn repr(byte: &u8) -> String {
    let btm = byte & 0b00001111;
    let top = (byte & 0b11110000) >> 4;

    return format!("{}{}", b_repr(top), b_repr(btm));
}

impl Runtime<'_> {
    fn load<'a>(bootstrap: &'a Vec<u8>, rom: &'a Vec<u8>) -> Runtime<'a> {
        return Runtime {
            cpu: CpuRegisters {
                ra: 0, rf: 0,
                rb: 0, rc: 0,
                rd: 0, re: 0,
                rh: 0, rl: 0,
                pc: 0,
                sp: 0,
            },
            rom: rom,
            bootstrap: bootstrap,
        };
    }

    fn tick(&mut self) {
        let opcode = self.get(self.cpu.pc);
        // https://meganesu.github.io/generate-gb-opcodes/
        println!("Opcode {}: {:?}", repr(&opcode), self.cpu);
        let cycles = match opcode {
            0x0 => { // NOP
                self.cpu.pc += 1;
                1
            },
            0x21 => {
                self.cpu.rl = self.get(self.cpu.pc + 1);
                self.cpu.rh = self.get(self.cpu.pc + 2);
                self.cpu.pc += 3;
                3
            },
            0x31 => { // jr nc, s8
                let b0 = self.get(self.cpu.pc + 1);
                let b1 = self.get(self.cpu.pc + 2);
                self.cpu.sp = ((b1 as u16) << 8) + (b0 as u16);
                self.cpu.pc += 3;
                3
            },
            0x32 => {
                let mut addr: u16 = ((self.cpu.rh as u16) << 8) + self.cpu.rl as u16;
                self.set(addr, self.cpu.ra);
                addr -= 1;
                self.cpu.rl = (addr & 0b11111111) as u8;
                self.cpu.rh = (addr >> 8) as u8;
                self.cpu.pc += 1;
                2
            },
            0xAF => {
                self.cpu.ra ^= self.cpu.ra;
                self.cpu.rf &= 1 << 7;
                self.cpu.rf |= ((self.cpu.ra == 0) as u8) << 7;

                self.cpu.pc += 1;
                1
            },
            0xCB => {
                let nextop = self.get(self.cpu.pc + 1);
                panic!("ERROR: Opcode CB{} not implemented!", repr(&nextop));
            },
            _ => {
                panic!("ERROR: Opcode {} not implemented!", repr(&opcode));
            },
        };
    }
    fn tick_CB(&mut self) -> u8 {
        0
    }

    fn get(&self, addr: u16) -> u8 {
        return self.bootstrap[addr as usize];
    }

    fn set(&self, addr: u16, val: u8) -> () {
    }
}

fn load_rom(filename: &str) -> Vec<u8> {
    let mut f = fs::File::open(filename).expect(&format!(
        "File `{}´ not found in current working directory",
        filename
    ));
    let meta = f
        .metadata()
        .expect(&format!("Unable to read `{}´ metadata", filename));

    let mut rom = vec![0; meta.len() as usize];
    f.read(&mut rom).expect("Overflow");
    return rom;
}

fn main() {
    let game_rom = load_rom("rom.gb");
    let bootstrap = load_rom("DMG_ROM.bin");

    let mut rt = Runtime::load(&bootstrap, &game_rom);

    loop {
        rt.tick();
    }
}
