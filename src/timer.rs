use crate::byteop::*;
use crate::memory::Memory;

pub struct Timer {
    internal_ticks: u16,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            internal_ticks: 0,
        }
    }

    pub fn tick(&mut self, mem: &mut impl Memory, ticks: u8) {
        self.internal_ticks = self.internal_ticks.wrapping_add(ticks as u16);

        let tima = mem.get(0xFF05);
        let tma = mem.get(0xFF06);
        let tac = mem.get(0xFF07);

        let clock_speed = tac & 0b11;
        let cc = match clock_speed {
            0b00 => 1024,
            0b01 => 16,
            0b10 => 64,
            0b11 => 256,
            _ => panic!("Unhandled clock speed configuration"),
        };

        if get_bit(tac, 2) == 0x1 {
            if tima == 0xFF {
                mem.set(0xFF04, tma);
                let interrupt_flag = mem.get(0xFF0F) | 0b100;
                mem.set(0xFF0F, interrupt_flag);
            } else {
                mem.set(0xFF04, tima + 1);
            }
        }
    }
}
