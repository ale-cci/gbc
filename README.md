# On MacOS
```bash
brew install SDL2 SDL2_image SDL2_ttf
export LIBRARY_PATH="$LIBRARY_PATH:/opt/homebrew/Cellar/sdl2/2.28.5/lib/"
```

# What i have read so far:
- About the PPU: https://hacktix.github.io/GBEDG/ppu/#an-introduction

- Abount Memory Bank Switch:
    - https://retrocomputing.stackexchange.com/questions/11732/how-does-the-gameboys-memory-bank-switching-work
    - https://b13rg.github.io/Gameboy-MBC-Analysis/#cart-1

- GB Boot sequence:
    - https://realboyemulator.wordpress.com/2013/01/03/a-look-at-the-game-boy-bootstrap-let-the-fun-begin/

- More technical refrerence: https://gekkio.fi/files/gb-docs/gbctr.pdf

- CPU DAA Instruction:
    - https://ehaskins.com/2018-01-30%20Z80%20DAA/
    - https://forums.nesdev.org/viewtopic.php?t=15944

- CPU Half Carry: https://gist.github.com/meganesu/9e228b6b587decc783aa9be34ae27841

- CPU Opcodes: https://meganesu.github.io/generate-gb-opcodes/

- Wierd behavior of the half-carry flag? https://stackoverflow.com/questions/57958631/game-boy-half-carry-flag-and-16-bit-instructions-especially-opcode-0xe8


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
- Really useful to debug cpu opcodes: https://github.com/robert/gameboy-doctor

# TODO:
- A lot
