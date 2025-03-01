#[allow(dead_code)]
use crate::byteop::*;
use crate::registers::*;
use crate::{memory::Memory, runtime::Runtime};
use sdl2::audio::{AudioCallback, AudioSpecDesired};

const CHAN_LEFT: usize = 0;
const CHAN_RIGHT: usize = 1;

pub struct APU {
    pub spec: AudioSpecDesired,

    master_volume: f32,
    chan_volume: [f32; 2],
    voice1: Voice1,
    voice2: Voice2,
    voice3: Voice3,
    voice4: Voice4,
}

fn wave_duty_lookup(value: u8) -> f32 {
    return match value {
        0 => 0.125,
        1 => 0.25,
        2 => 0.5,
        3 => 0.75,
        _ => panic!("Value {value} not valid"),
    };
}

impl APU {
    pub fn new() -> Self {
        const CHANNELS: u8 = 2;

        APU {
            spec: AudioSpecDesired {
                freq: Some(44100),
                channels: Some(CHANNELS),
                samples: None,
            },
            chan_volume: [0.0, 0.0],

            master_volume: 0.0,
            voice1: Voice1::default(),
            voice2: Voice2::default(),
            voice3: Voice3::default(),
            voice4: Voice4::default(),
        }
    }

    pub fn update(&mut self, ticks: u8, rt: &mut Runtime) {
        let nr52 = rt.get(NR52);
        let audio_on = get_bit(nr52, 7);

        if audio_on == 1 {
            self.master_volume = 1.0;
        } else {
            self.master_volume = 0.0;
        }

        let nr50 = rt.get(NR50);

        // volume 0 is 1/8, vol 8 is 1
        self.chan_volume[CHAN_LEFT] = (((nr50 & 0b1110000) >> 4) as f32 + 1.0) / 8.0;
        self.chan_volume[CHAN_RIGHT] = ((nr50 & 0b111) as f32 + 1.0) / 8.0;

        self.voice1.tick(ticks, rt);
        self.voice2.tick(ticks, rt);
        self.voice3.tick(ticks, rt);
        self.voice4.tick(ticks, rt);

        let mut mask = 0;
        if self.voice1.is_active() {
            mask |= 0b0001;
        }
        if self.voice2.is_active() {
            mask |= 0b0010;
        }
        if self.voice3.is_active() {
            mask |= 0b0100;
        }
        if self.voice4.is_active() {
            mask |= 0b1000;
        }
        rt.set(NR52, rt.get(NR52) | mask);
    }
}

#[derive(Default)]
struct Voice1 {
    phase: f32,
    pace: u8,
    direction: u8,
    step: u8,
    wave_duty: u8,
    length: u8,
    volume: u8,
    envelope: u8,

    sweep: u8,

    period: u16,
    period_sweeps: u8,
    length_enable: bool,
    trigger: bool,

    chan_volume: [f32; 2],
    on: bool,
    dac_on: bool,

    sweep_vol_timer: i16,
    sweep_len_timer: i16,
    sweep_freq_timer: i16,
}

trait BitChannel {
    // says if the channel is active or not, i.e. if it produces any sound.
    // calling "overlap" on non active channels does nothing.
    fn is_active(&self) -> bool;

    // adds the output wave of this channel to "out".
    fn overlap(&mut self, out: &mut [f32], channels: usize);

    // updates the channel, reading what it should output from memory.
    fn tick(&mut self, ticks: u8, rt: &mut Runtime);
}

impl BitChannel for Voice1 {
    fn is_active(&self) -> bool {
        self.on && self.dac_on
    }

    fn tick(&mut self, ticks: u8, rt: &mut Runtime) {
        self.dac_on = (rt.get(NR12) & 0xf8) != 0;

        if self.on & self.dac_on {
            if self.length_enable {
                // DIV_APU increments by one every 16 ticks (every time bit 4 of div goes from 1 to
                // 0)
                // sound length occurs every 2 DIV-APU ticks, so it triggers every 16 * 2 ticks
                self.sweep_len_timer -= rt.timer.delta_div as i16;

                while self.sweep_len_timer <= 0 {
                    self.sweep_len_timer += 2;
                    if self.length < 64 {
                        self.length += 1;
                    } else {
                        self.on = false;
                    }
                }
            }

            if self.pace != 0 {
                self.sweep_freq_timer -= rt.timer.delta_div as i16;
                while self.sweep_freq_timer <= 0 {
                    self.sweep_freq_timer += 4 * (self.pace as i16);
                    let delta = self.period / (1 << self.step);
                    if self.direction > 0 {
                        self.period = self.period + delta;
                    } else {
                        self.period = self.period - delta;
                    }
                    if self.period > 0x7FF {
                        self.on = false;
                    }

                    // write back nr14 & nr13
                    rt.hwset(NR13, (self.period & 0xF) as u8);

                    let mask = (self.period >> 8) as u8 & 0b111;
                    rt.hwset(NR14, rt.get(NR14) | mask);
                }
            }

            if self.sweep != 0 {
                self.sweep_vol_timer -= rt.timer.delta_div as i16;
                while self.sweep_vol_timer <= 0 {
                    self.sweep_vol_timer += 8 * (self.sweep as i16);

                    if self.envelope > 0 && self.volume < 0xF {
                        self.volume += 1;
                    } else if self.envelope == 0 && self.volume > 0 {
                        // pandocs: "the envelope reaching a volume of 0 does NOT turn the channel
                        // off."
                        self.volume -= 1;
                    }

                }
            }
        }

        let nr10 = rt.get(NR10);
        let nr11 = rt.get(NR11);
        let nr12 = rt.get(NR12);
        let nr13 = rt.get(NR13);
        let nr14 = rt.get(NR14);
        let nr51 = rt.get(NR51);

        let trigger = get_bit(nr14, 7) == 1;

        self.period = nr13 as u16 + ((nr14 as u16 & 0b111) << 8); // frequency
        self.wave_duty = (nr11 & 0b11000000) >> 6;
        self.step = nr10 & 0b111;
        self.sweep = nr12 & 0b111; // envelope period
        self.direction = get_bit(nr10, 3);
        self.pace = (nr10 & 0b1110000) >> 4;
        self.chan_volume = [get_bit(nr51, 4) as f32, get_bit(nr51, 0) as f32];
        self.volume = (nr12 & 0b11110000) >> 4;

        if trigger && !self.on {
            self.period_sweeps = 0;
            // update voice 1
            self.sweep_freq_timer = self.pace as i16 * 4; // 128hz

            self.phase = 0.0;

            self.length = nr11 & 0b111111;
            self.envelope = get_bit(nr12, 3);
            self.sweep_vol_timer = 8 * self.sweep as i16; // 64hz

            self.length_enable = get_bit(nr14, 6) == 1;
            self.sweep_len_timer = 2; // 256hz

            self.on = trigger;
            rt.hwset(NR14, set_bit(nr14, 7, false));
        }



    }

    fn overlap(&mut self, out: &mut [f32], channels: usize) {
        if !self.is_active() {
            return;
        }

        let phase_increment = pi_from_period(self.period);
        let freq = 1.0 / phase_increment;

        // println!("ch1: freq({}) vol({}) env({}) duty({})", self.period, self.volume, self.sweep, self.wave_duty);
        // println!("hz: {}", freq);
        let duty = wave_duty_lookup(self.wave_duty);
        let volume = self.volume as f32 / 15.0;

        let phase_increment = freq / 44100.0;


        for (i, x) in out.iter_mut().enumerate() {
            *x = if self.phase < duty { volume } else { -volume };

            if i % channels == (channels - 1) {
                self.phase = (self.phase + phase_increment) % 1.0;
            }
        }
    }
}

#[derive(Default)]
struct Voice2 {
    phase: f32,
    wave_duty: u8,
    length: u8,
    volume: u8,
    envelope: u8,
    sweep: u8,

    period: u16,
    length_enable: bool,
    trigger: bool,

    chan_volume: [f32; 2],

    on: bool,
    dac_on: bool,

    sweep_len_timer: i16,
}

impl BitChannel for Voice2 {
    fn is_active(&self) -> bool {
        self.on && self.dac_on
    }
    fn overlap(&mut self, out: &mut [f32], channels: usize) {
        if !self.is_active() {
            return;
        }
        let phase_increment = pi_from_period(self.period);
        let freq = 1.0 / phase_increment;

        let duty = wave_duty_lookup(self.wave_duty);
        let volume = self.volume as f32 / 15.0;

        let phase_increment = freq / 44100.0;


        for (i, x) in out.iter_mut().enumerate() {
            *x = if self.phase < duty { volume } else { -volume };

            if i % channels == (channels - 1) {
                self.phase = (self.phase + phase_increment) % 1.0;
            }
        }

    }

    fn tick(&mut self, ticks: u8, rt: &mut Runtime) {
        self.dac_on = (rt.get(NR22) & 0xF8) != 0;

        if self.on {
            if self.length_enable {
                // DIV_APU increments by one every 16 ticks (every time bit 4 of div goes from 1 to
                // 0)
                // sound length occurs every 2 DIV-APU ticks, so it triggers every 16 * 2 ticks
                self.sweep_len_timer -= rt.timer.delta_div as i16;

                while self.sweep_len_timer <= 0 {
                    self.sweep_len_timer += 2;
                    if self.length < 64 {
                        self.length += 1;
                    } else {
                        self.on = false;
                    }
                }
            }
        }

        let nr21 = rt.get(NR21);
        let nr22 = rt.get(NR22);
        let nr23 = rt.get(NR23);
        let nr24 = rt.get(NR24);
        let nr51 = rt.get(NR51);

        let trigger = get_bit(nr24, 7) == 1;

        self.chan_volume = [get_bit(nr51, 5) as f32, get_bit(nr51, 1) as f32];
        self.wave_duty = (nr21 & 0b11000000) >> 6;
        self.length = nr21 & 0b111111;
        self.volume = (nr22 & 0b11110000) >> 4;
        self.envelope = get_bit(nr22, 3);
        self.sweep = nr22 & 0b111;
        self.period = nr23 as u16 + ((nr24 as u16 & 0b111) << 8);

        if trigger && !self.on {

            self.sweep_len_timer = 2;
            self.length_enable = get_bit(nr24, 6) == 1;
            if !self.trigger {
                self.trigger = get_bit(nr24, 7) == 1;
                if self.trigger {
                    rt.hwset(NR24, set_bit(nr24, 7, false))
                }
            }

            self.on = true;
            rt.hwset(NR24, set_bit(nr24, 7, false));
        }


    }
}

#[derive(Default)]
struct Voice3 {
    dac: bool,
    phase: f32,
    length_enable: bool,
    length: u8,
    trigger: bool,
    volume: f32,
    period: u16,
    pattern: [u8; 16],
}
impl BitChannel for Voice3 {
    fn is_active(&self) -> bool {
        return false;
    }
    fn overlap(&mut self, out: &mut [f32], channels: usize) {
        if !self.is_active() {
            return;
        }
        println!("voice3 on");
    }
    fn tick(&mut self, ticks: u8, rt: &mut Runtime) {
        let nr34 = rt.get(NR34);
        self.dac = get_bit(rt.get(NR30), 7) == 1;
        self.length = rt.get(NR31);
        self.volume = match (rt.get(NR32) & 0b1100000) >> 5 {
            0 => 0.0,
            1 => 1.0,
            2 => 0.5,
            3 => 0.25,
            v => panic!("Value {v} not allowed"),
        };
        self.period = rt.get(NR33) as u16 + ((nr34 as u16 & 0b111) << 8);
        self.length_enable = get_bit(nr34, 6) == 1;

        if !self.trigger {
            self.trigger = get_bit(nr34, 7) == 1;
            if self.trigger {
                rt.hwset(NR34, set_bit(nr34, 7, false));
            }
        }

        for (i, addr) in (0xFF30..=0xFF3F).enumerate() {
            self.pattern[i] = rt.get(addr);
        }
    }
}

#[derive(Default)]
struct Voice4 {
    phase: f32,
    trigger: bool,
}

impl BitChannel for Voice4 {
    fn is_active(&self) -> bool {
        return false;
    }

    fn overlap(&mut self, out: &mut [f32], channels: usize) {
        if !self.is_active() {
            return;
        }
    }

    fn tick(&mut self, ticks: u8, rt: &mut Runtime) {}
}

fn pi_from_period(period: u16) -> f32 {
    let v = (1048576u32 / (2048 - period as u32) / 8) as f32;
    return 1.0 / v;
}

impl AudioCallback for &mut APU {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let channels = self.spec.channels.unwrap().into();

        for x in out.iter_mut() {
            *x = 0.0;
        }

        self.voice1.overlap(out, channels);
        self.voice2.overlap(out, channels);
        self.voice3.overlap(out, channels);
        self.voice4.overlap(out, channels);

        for (i, x) in out.iter_mut().enumerate() {
            *x = *x * self.chan_volume[i % 2 as usize] * self.master_volume;
        }
    }
}
