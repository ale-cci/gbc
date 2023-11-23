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
