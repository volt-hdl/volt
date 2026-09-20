# Builds hello.hex, the $readmemh image the test loads into the SoC's
# program memory with read_hex() + load(). It is committed, so
# `volt test` works without a RISC-V toolchain; run `make` only after
# changing the program.
#
# Debian/Ubuntu ship the compiler as riscv64-unknown-elf-gcc; it targets
# RV32 through -march/-mabi. With a riscv32 toolchain:
#     make CROSS=riscv32-unknown-elf-

CROSS   ?= riscv64-unknown-elf-
CC      := $(CROSS)gcc
OBJCOPY := $(CROSS)objcopy
OBJDUMP := $(CROSS)objdump

# zicsr: binutils >= 2.38 no longer implies the CSR instructions
# (start.S writes mtvec) from plain rv32im.
CFLAGS := -march=rv32im_zicsr -mabi=ilp32 -O2 -Wall -Wextra \
          -ffreestanding -fno-builtin -nostdlib -nostartfiles \
          -Wl,--build-id=none -T link.ld

all: hello.hex

hello.elf: start.S hello.c link.ld
	$(CC) $(CFLAGS) start.S hello.c -o $@

# 32-bit words, one per memory element. objcopy keeps BYTE addresses
# in the '@' lines even with a 4-byte data width, while $readmemh counts
# elements — harmless here because the image starts at 0.
hello.hex: hello.elf
	$(OBJCOPY) -O verilog --verilog-data-width=4 $< $@

# Disassembly, for reading along with a waveform.
dump: hello.elf
	$(OBJDUMP) -d -M no-aliases $<

clean:
	rm -f hello.elf

.PHONY: all dump clean
