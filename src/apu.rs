use crate::byteop::*;
#[allow(dead_code)]
use crate::memory::Memory;
use crate::registers::*;
use sdl2::audio::{AudioCallback, AudioSpecDesired};

const CHAN_LEFT: usize = 0;
const CHAN_RIGHT: usize = 1;

pub struct APU {
    pub spec: AudioSpecDesired,
    phase: f32,

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
            phase: 0.0,
            chan_volume: [0.0, 0.0],

            master_volume: 0.0,
            voice1: Voice1::default(),
            voice2: Voice2::default(),
            voice3: Voice3::default(),
            voice4: Voice4::default(),
        }
    }

    fn voice1(&self, phase: f32, channel: u8) -> f32 {
        let volume = self.voice1.volume as f32 / 15.0;
        let volume = volume * self.voice1.chan_volume[channel as usize] * self.master_volume;
        if volume == 0.0 {
            return 0.0;
        }

        let duty = wave_duty_lookup(self.voice1.wave_duty);
        return if phase < duty { volume } else { -volume };
    }

    fn voice2(&self, phase: f32, channel: u8) -> f32 {
        let volume = self.voice2.volume as f32 / 15.0;
        let volume = volume * self.voice2.chan_volume[channel as usize] * self.master_volume;
        if volume == 0.0 {
            return 0.0;
        }

        let duty = wave_duty_lookup(self.voice2.wave_duty);
        return if phase < duty { volume } else { -volume };
    }

    fn voice3(&self, phase: f32, chan: u8) -> f32 {
        0.0
    }
    fn voice4(&self, phase: f32, chan: u8) -> f32 {
        0.0
    }

    pub fn update(&mut self, ticks: u8, rt: &mut impl Memory) {
        let nr52 = rt.get(NR52);
        let audio_on = get_bit(nr52, 7);
        if audio_on == 1 {
            self.master_volume = 1.0;
        } else {
            self.master_volume = 0.0;
        }

        let nr50 = rt.get(NR50);

        // 0 is 1/8
        self.chan_volume[CHAN_LEFT] = (((nr50 & 0b1110000) >> 4) as f32 + 1.0) / 8.0;
        self.chan_volume[CHAN_RIGHT] = ((nr50 & 0b111) as f32 + 1.0) / 8.0;

        self.voice1.tick(ticks, rt);
        self.voice2.tick(ticks, rt);


        // update voice 3
        let nr34 = rt.get(NR34);

        self.voice3.dac = get_bit(rt.get(NR30), 7) == 1;
        self.voice3.length = rt.get(NR31);
        self.voice3.volume = match (rt.get(NR32) & 0b1100000) >> 5 {
            0 => 0.0,
            1 => 1.0,
            2 => 0.5,
            3 => 0.25,
            v => panic!("Value {v} not allowed"),
        };
        self.voice3.period = rt.get(NR33) as u16 + ((nr34 as u16 & 0b111) << 8);
        self.voice3.length_enable = get_bit(nr34, 6) == 1;

        if !self.voice3.trigger {
            self.voice3.trigger = get_bit(nr34, 7) == 1;
            if self.voice3.trigger {
                rt.hwset(NR34, set_bit(nr34, 7, false));
            }
        }

        for (i, addr) in (0xFF30..=0xFF3F).enumerate() {
            self.voice3.pattern[i] = rt.get(addr);
        }
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
    sweep_timer: u8,

    period: u16,
    length_enable: bool,
    trigger: bool,

    chan_volume: [f32; 2],
}

impl Voice1 {
    fn is_active(&self) -> bool {
        self.trigger || self.length_enable && self.length < 64
    }

    fn tick(&mut self, ticks: u8, rt:  &mut impl Memory) {
        let playing = self.trigger || self.length_enable && self.length < 64;

        if playing {
            if self.length_enable && self.length < 64 {
                self.length += 1;
            }

            if self.sweep != 0 {
                self.sweep_timer -= 1;

                if self.sweep_timer == 0 {
                    self.sweep_timer = self.sweep;

                    if self.envelope == 1 && self.volume < 0xF{
                        self.volume += 1;
                    } else if self.envelope == 0 && self.volume > 0 {
                        self.volume -= 1;
                    }
                }
            }
        } else {
            let nr10 = rt.get(NR10);
            let nr11 = rt.get(NR11);
            let nr12 = rt.get(NR12);
            let nr13 = rt.get(NR13);
            let nr14 = rt.get(NR14);

            // update voice 1
            self.pace = (nr10 & 0b1110000) >> 4;
            self.direction = get_bit(nr10, 3);
            self.step = nr10 & 0b111;
            self.wave_duty = (nr11 & 0b11000000) >> 6;
            self.phase = 0.0;

            self.length = nr11 & 0b111111;
            self.volume = (nr12 & 0b11110000) >> 4;
            self.envelope = get_bit(nr12, 3);
            self.sweep = nr12 & 0b111;
            self.sweep_timer = self.sweep;

            self.length_enable = get_bit(nr14, 6) == 1;
            if !self.trigger {
                self.trigger = get_bit(nr14, 7) == 1;
                if self.trigger {
                    rt.hwset(NR14, set_bit(nr14, 7, false))
                }
            }
            self.period = nr13 as u16 + ((nr14 as u16 & 0b111) << 8);

            let nr51 = rt.get(NR51);
            self.chan_volume = [
                get_bit(nr51, 4) as f32,
                get_bit(nr51, 0) as f32
            ];
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
}

impl Voice2 {
    fn is_active(&self) -> bool {
        self.trigger || self.length_enable && self.length < 64
    }

    fn tick(&mut self, ticks: u8, rt: &mut impl Memory) {
        if self.length_enable && self.length < 64 {
            self.length += 1;
        } else {
            let nr21 = rt.get(NR21);
            let nr22 = rt.get(NR22);
            let nr23 = rt.get(NR23);
            let nr24 = rt.get(NR24);

            self.wave_duty = (nr21 & 0b11000000) >> 6;
            self.length = nr21 & 0b111111;
            self.volume = (nr22 & 0b11110000) >> 4;
            self.envelope = get_bit(nr22, 3);
            self.sweep = nr22 & 0b111;

            self.length_enable = get_bit(nr24, 6) == 1;
            if !self.trigger {
                self.trigger = get_bit(nr24, 7) == 1;
                if self.trigger {
                    rt.hwset(NR24, set_bit(nr24, 7, false))
                }
            }
            self.period = nr23 as u16 + ((nr24 as u16 & 0b111) << 8);
            let nr51 = rt.get(NR51);
            self.chan_volume = [
                get_bit(nr51, 5) as f32,
                get_bit(nr51, 1) as f32,
            ];
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

#[derive(Default)]
struct Voice4 {
    phase: f32,
    trigger: bool,
}

fn pi_from_period(period: u16) -> f32 {
    let v = (1048576u32 / (2048 - period as u32) / 32) as f32;

    return 1.0 / v;
}

impl AudioCallback for &mut APU {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let phase1_inc = pi_from_period(self.voice1.period);
        let phase2_inc = pi_from_period(self.voice2.period);
        let phase3_inc = pi_from_period(self.voice3.period);

        // let hz = phase_inc * 44100.0;
        // println!("Hz: {hz}Hz");

        let channels = 2;
        let mut chan = 0;
        for x in out.iter_mut() {
            chan = (chan + 1) % channels;
            let mut output = 0.0;

            if self.voice1.is_active() {
                output += self.voice1(self.voice1.phase, chan);
            }

            if self.voice2.is_active() {
                output += self.voice2(self.voice2.phase, chan);
            }
            if self.voice3.trigger {
                output += self.voice3(self.voice3.phase, chan);
            }
            output += self.voice4(self.phase, chan);

            *x = output * self.chan_volume[chan as usize];

            if chan == 1 {
                self.voice1.phase = (self.voice1.phase + phase1_inc) % 1.0;
                self.voice2.phase = (self.voice2.phase + phase2_inc) % 1.0;
                self.voice3.phase = (self.voice3.phase + phase3_inc) % 1.0;
            }
        }

        self.voice1.trigger = false;
        self.voice2.trigger = false;
        self.voice3.trigger = false;
        self.voice4.trigger = false;
    }
}
