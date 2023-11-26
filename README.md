# On MacOS
```bash
brew install SDL2 SDL2_image SDL2_ttf
export LIBRARY_PATH="$LIBRARY_PATH:/opt/homebrew/Cellar/sdl2/2.28.5/lib/"
```

# What i have read so far:
- [GB Boot sequence](https://realboyemulator.wordpress.com/2013/01/03/a-look-at-the-game-boy-bootstrap-let-the-fun-begin/)
- [More technical refrerence](https://gekkio.fi/files/gb-docs/gbctr.pdf)
- [List of CPU Opcodes](https://meganesu.github.io/generate-gb-opcodes/)
- [CPU Carry & Half Carry](https://gist.github.com/meganesu/9e228b6b587decc783aa9be34ae27841)
- [Wierd behavior of the half-carry flag?](https://stackoverflow.com/questions/57958631/game-boy-half-carry-flag-and-16-bit-instructions-especially-opcode-0xe8)

- CPU DAA Instruction explained: [here](https://ehaskins.com/2018-01-30%20Z80%20DAA/) and [here](https://forums.nesdev.org/viewtopic.php?t=15944)

- About the PPU: 
    - [gbedg](https://hacktix.github.io/GBEDG/ppu/#an-introduction)
    - [pandocs](https://gbdev.io/pandocs/Graphics.html)

- Abount Memory Bank Switch:
    - https://retrocomputing.stackexchange.com/questions/11732/how-does-the-gameboys-memory-bank-switching-work
    - https://b13rg.github.io/Gameboy-MBC-Analysis/#cart-1

- [The Ultimate Game Boy Talk](https://www.youtube.com/watch?v=HyzD8pNlpwI)

# Blargg rom status:
- [x] instr timing
- [ ] cpu instrs (full requires implementation of MB1)
    - [x] 01 - special
    - [x] 02 - interrupts
    - [x] 03 - op sp, hl
    - [x] 04 - op r, imm
    - [x] 05 - op rp
    - [x] 06 - ld r,r
    - [x] 07 - jr, jp, call, ret, rst
    - [x] 08 - misc instrs
    - [x] 09 - op r,r
    - [x] 10 - bit ops
    - [x] 11 - op a, (hl)
- [ ] interrupt time
- [ ] cgb sound
- [ ] mem timing
- [ ] mem timing-2
- [ ] halt bug

# NOTES:
- A basic joypad implementation is required to display tetris screen.
- Debug CPU Opcodes:
    - https://github.com/retrio/gb-test-roms
    - https://github.com/robert/gameboy-doctor


# TODO:
- A lot
