pub fn join_u8(h: u8, l: u8) -> u16 {
    return ((h as u16) << 8) + l as u16;
}

pub fn split_u16(hl: u16) -> (u8, u8) {
    let l = (hl & 0b11111111) as u8;
    let h = (hl >> 8) as u8;
    return (h, l);
}

/// Get the nth bit of the register
pub fn get_bit<T: Into<u16>>(reg: T, pos: u8) -> u8 {
    let reg = reg.into();
    let bit = (reg & (1 << pos as u16)) >> pos as u16;
    return bit as u8;
}

pub fn set_bit<T: Into<bool>>(reg: u8, pos: u8, val: T) -> u8 {
    let mask: u8 = 1 << pos;

    return if val.into() {
        reg | mask
    } else {
        reg & !mask
    }
}


fn b_repr(byte: u8) -> char {
    if byte <= 9 {
        return (byte + '0' as u8) as char;
    }
    return (byte - 10 + ('A' as u8)) as char;
}


pub fn b64<T: Into<u16>>(arg: T) -> String {
    let iter = std::mem::size_of::<T>();
    let mut chars = vec![];

    let mut num = arg.into() as u16;
    for _ in 0..iter {
        let l = b_repr((num as u8) & 0b00001111);
        chars.push(l);
        let h = b_repr((num as u8 & 0b11110000) >> 4);
        chars.push(h);

        num >>= 8;
    }

    return String::from_iter(chars.into_iter().rev());
}

pub fn rl(cy: u8, reg: u8) -> (u8, u8) {
    let msb = get_bit(reg, 7);
    return (
        msb,
        (reg << 1) + cy,
    )
}

pub fn res(reg: &mut u8, pos: u8) -> u8 {
    *reg = set_bit(*reg, pos, false);
    2
}

pub fn set(reg: &mut u8, pos: u8) -> u8 {
    *reg = set_bit(*reg, pos, true);
    2
}

