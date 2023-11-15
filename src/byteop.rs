pub fn join_u8(h: u8, l: u8) -> u16 {
    return ((h as u16) << 8) + l as u16;
}

pub fn split_u16(hl: u16) -> (u8, u8) {
    let l = (hl & 0b11111111) as u8;
    let h = (hl >> 8) as u8;
    return (h, l);
}

pub fn get_bit(reg: u8, pos: u8) -> u8 {
    return (reg & (1 << pos)) >> pos;
}
