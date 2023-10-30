R=rustkernel

KERNEL_SOURCES = \
  entry.c \
  proc.c \
  swtch.c \
  trampoline.c \
  bio.c \
  fs.c \
  log.c \
  exec.c \
  sysfile.c \
  kernelvec.c \
  virtio_disk.c

OBJS = $(KERNEL_SOURCES:%.c=%.o)

# riscv64-unknown-elf- or riscv64-linux-gnu-
# perhaps in /opt/riscv/bin
#TOOLPREFIX = 

# Try to infer the correct TOOLPREFIX if not set
ifndef TOOLPREFIX
TOOLPREFIX := $(shell if riscv64-unknown-elf-objdump -i 2>&1 | grep 'elf64-big' >/dev/null 2>&1; \
	then echo 'riscv64-unknown-elf-'; \
	elif riscv64-linux-gnu-objdump -i 2>&1 | grep 'elf64-big' >/dev/null 2>&1; \
	then echo 'riscv64-linux-gnu-'; \
	elif riscv64-unknown-linux-gnu-objdump -i 2>&1 | grep 'elf64-big' >/dev/null 2>&1; \
	then echo 'riscv64-unknown-linux-gnu-'; \
	else echo "***" 1>&2; \
	echo "*** Error: Couldn't find a riscv64 version of GCC/binutils." 1>&2; \
	echo "*** To turn off this error, run 'gmake TOOLPREFIX= ...'." 1>&2; \
	echo "***" 1>&2; exit 1; fi)
endif

CC = $(TOOLPREFIX)gcc
LD = $(TOOLPREFIX)ld
OBJCOPY = $(TOOLPREFIX)objcopy
# OBJDUMP = $(TOOLPREFIX)objdump

CFLAGS = -Wall -Werror -O -fno-omit-frame-pointer -ggdb -gdwarf-2
CFLAGS += -MD
CFLAGS += -mcmodel=medany
CFLAGS += -ffreestanding -fno-common -nostdlib -mno-relax
CFLAGS += -I.
CFLAGS += $(shell $(CC) -fno-stack-protector -E -x c /dev/null >/dev/null 2>&1 && echo -fno-stack-protector)

TARGET_TRIPLE = riscv64gc-unknown-none-elf
RUST_LIB = $R/target/$(TARGET_TRIPLE)/release/librustkernel.a

# Disable PIE when possible (for Ubuntu 16.10 toolchain)
ifneq ($(shell $(CC) -dumpspecs 2>/dev/null | grep -e '[^f]no-pie'),)
CFLAGS += -fno-pie -no-pie
endif
ifneq ($(shell $(CC) -dumpspecs 2>/dev/null | grep -e '[^f]nopie'),)
CFLAGS += -fno-pie -nopie
endif

LDFLAGS = -z max-page-size=4096

.PHONY: clean

kernel: $(OBJS) kernel.ld initcode $(RUST_LIB)
	$(LD) $(LDFLAGS) -T kernel.ld -o kernel $(OBJS) $(RUST_LIB)
	# $(OBJDUMP) -S kernel > kernel.asm
	# $(OBJDUMP) -t kernel | sed '1,/SYMBOL TABLE/d; s/ .* / /; /^$$/d' > kernel.sym

$(RUST_LIB): $(shell find $R/src -type f) $R/Cargo.toml
	cargo +nightly -Z unstable-options -C $R build --release
	# $(OBJDUMP) -S $(RUST_LIB) > $(RUST_LIB).asm

initcode: initcode.S
	$(CC) $(CFLAGS) -march=rv64g -nostdinc -I. -Ikernel -c initcode.S -o initcode.o
	$(LD) $(LDFLAGS) -N -e start -Ttext 0 -o initcode.out initcode.o
	$(OBJCOPY) -S -O binary initcode.out initcode
	# $(OBJDUMP) -S initcode.o > initcode.asm

%.o: %.c *.h
	$(CC) $(CFLAGS) -c $<

clean: 
	rm -f *.tex *.dvi *.idx *.aux *.log *.ind *.ilg \
	*.o *.a *.d *.asm *.sym *.gch \
	initcode initcode.out kernel
	cargo +nightly -Z unstable-options -C $R clean
