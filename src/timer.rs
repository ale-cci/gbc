use crate::byteop::*;
use crate::memory::Memory;

pub struct Timer {
    internal_ticks: u16,

    pub delta_div: u8,
}

const INT_ADDR: u16 = 0xFF0F;
const TIMA_ADDR: u16 = 0xFF05;
const TMA_ADDR: u16 = 0xFF06;
const DIV_ADDR: u16 = 0xFF04;
const TAC_ADDR: u16 = 0xFF07;

impl Timer {
    pub fn new() -> Timer {
        Timer {
            internal_ticks: 0,
            delta_div: 0,
        }
    }

    pub fn tick(&mut self, mem: &mut impl Memory, ticks: u8) {
        let internal_ticks = self.internal_ticks;
        self.internal_ticks = self.internal_ticks.wrapping_add(ticks as u16);

        let div = mem.get(DIV_ADDR);

        let timer_incr = timer_increment(internal_ticks, ticks, 3);

        // apu tick every time bit 4 goes from 1 to 0
        self.delta_div = ((div & 0b11111) + timer_incr) >> 5;

        let div = div.wrapping_add(timer_incr);

        mem.hwset(DIV_ADDR, div as u8);

        let tima = mem.get(TIMA_ADDR);
        let tma = mem.get(TMA_ADDR);
        let tac = mem.get(TAC_ADDR);

        let timer_speed = tac & 0b11;

        if get_bit(tac, 2) == 0x1 {
            let incr = timer_increment(internal_ticks, ticks, timer_speed);
            let interrupt = (tima as u16 + incr as u16) > 0xFF;

            let tima = if interrupt {
                let int_flag = mem.get(INT_ADDR) | 0b100;
                mem.set(INT_ADDR, int_flag);

                tma + ((incr - (0xFF - tima + 1)) & (0xFF - tma))
            } else {
                tima + incr
            };

            mem.set(TIMA_ADDR, tima);
        }
    }
}

fn timer_increment(curr_cycles: u16, elapsed: u8, speed: u8) -> u8 {
    let shifts = match speed {
        0 => 8 + 2, // 4x slower
        1 => 8 - 4, // 16x faster
        2 => 8 - 2, // 2x faster
        3 => 8,     // 1x (same as div)
        _ => panic!("Unhandled speed"),
    };

    let mask = (1 << shifts) - 1;
    let mut curr_cycles = curr_cycles & mask;
    curr_cycles += elapsed as u16;

    return (curr_cycles >> shifts) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mem {
        memory: Vec<u8>,
    }
    impl Mem {
        fn new() -> Mem {
            Mem {
                memory: vec![0; 0xFFFF],
            }
        }
    }
    impl Memory for Mem {
        fn get(&self, addr: u16) -> u8 {
            self.memory[addr as usize]
        }
        fn set(&mut self, addr: u16, val: u8) {
            self.memory[addr as usize] = val;
        }
        fn hwset(&mut self, addr: u16, val: u8) {
            self.memory[addr as usize] = val;
        }
    }

    #[test]
    fn test_div_increment() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();

        timer.tick(&mut mem, 255);
        assert_eq!(mem.get(DIV_ADDR), 0);

        timer.tick(&mut mem, 1);
        assert_eq!(mem.get(DIV_ADDR), 1);
    }

    #[test]
    fn test_invokes_interrupt() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();
        mem.set(TAC_ADDR, 0b111);

        timer.tick(&mut mem, 255);
        assert_eq!(mem.get(DIV_ADDR), 0);

        timer.tick(&mut mem, 1);
        assert_eq!(mem.get(DIV_ADDR), 1);
    }

    #[test]
    fn test_does_not_increment_tima_if_tac_enable_is_0() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();
        mem.set(TAC_ADDR, 0b011);
        timer.tick(&mut mem, 255);
        timer.tick(&mut mem, 1);

        let tima = mem.get(TIMA_ADDR);
        assert_eq!(tima, 0);
    }

    #[test]
    fn test_does_increment_tima_if_tac_enable_is_1() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();
        mem.set(TAC_ADDR, 0b111);
        timer.tick(&mut mem, 255);
        timer.tick(&mut mem, 1);

        let tima = mem.get(TIMA_ADDR);
        assert_eq!(tima, 1);
    }

    #[test]
    fn test_tima_restarts_from_tma() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();
        mem.set(TAC_ADDR, 0b111);
        mem.set(TIMA_ADDR, 0xFF);
        mem.set(TMA_ADDR, 0xFE);

        timer.tick(&mut mem, 255);
        timer.tick(&mut mem, 1);

        let tima = mem.get(TIMA_ADDR);
        assert_eq!(tima, 0xFE);
    }

    #[test]
    fn test_tima_restarts_from_tma_f0() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();
        mem.set(TAC_ADDR, 0b111);
        mem.set(TIMA_ADDR, 0xFF);
        mem.set(TMA_ADDR, 0xF0);

        timer.tick(&mut mem, 255);
        timer.tick(&mut mem, 1);

        let tima = mem.get(TIMA_ADDR);
        assert_eq!(tima, 0xF0);
    }

    #[test]
    fn test_tima() {
        let mut timer = Timer::new();
        let mut mem = Mem::new();
        mem.set(TAC_ADDR, 0b101);
        mem.set(TIMA_ADDR, 0);
        mem.set(INT_ADDR, 0);

        for _ in 0..4 {
            timer.tick(&mut mem, 250);
            timer.tick(&mut mem, 250);
        }
        timer.tick(&mut mem, 250);

        assert_eq!(mem.get(0xFF0F), 0);

        for _ in 0..4 {
            timer.tick(&mut mem, 250);
            timer.tick(&mut mem, 250);
        }

        assert_eq!(mem.get(0xFF0F), 0b100);
    }
}
