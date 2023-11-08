use std::fs;
use std::io::Read;
// https://retrocomputing.stackexchange.com/questions/11732/how-does-the-gameboys-memory-bank-switching-work
// https://realboyemulator.wordpress.com/2013/01/03/a-look-at-the-game-boy-bootstrap-let-the-fun-begin/

struct CpuRegisters {
    ra: u8,
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
                ra: 0,
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
        let cycles = match opcode {
            0x0 => { // NOP
                self.cpu.pc += 1;
                1
            },
            0x31 => { // jr nc, s8
                let b0 = self.get(self.cpu.pc + 1);
                let b1 = self.get(self.cpu.pc + 2);
                self.cpu.sp = ((b1 as u16) << 8) + (b0 as u16);
                self.cpu.pc += 3;
                3
            },
            0xAF => {
                self.cpu.pc += 1;
                1
            }
            _ => {
                println!("ERROR: Opcode {} not implemented!", repr(&opcode));
                0
            },
        };
    }

    fn get(&self, addr: u16) -> u8 {
        return self.bootstrap[addr as usize];
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
