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

    chars.push('x');
    chars.push('0');

    return String::from_iter(chars.into_iter().rev());
}
