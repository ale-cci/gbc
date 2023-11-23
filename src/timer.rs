trait Memory {
    fn get(&self) -> u8;
    fn set(&mut self, addr: u16);
}

struct Timer {
    internal_ticks: u16,
}

impl Timer {
    fn tick(&mut self, mem: &impl Memory) {
    }
}
