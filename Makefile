#
# Makefile - Top-level build, clean and flash for the Rust demo application
#
# Author:  Sandesh Ghimire <sandesh@soccentric.com>
# Target:  NUCLEO-F767ZI (STM32F767ZI)
#
# Everything runs in the stm32-dev-rust container (RCON-001), the same way as
# scripts/build.sh. Flashing and debugging use the same OpenOCD set-up as the
# C projects.
#
#   make                build the firmware (same as make build)
#   make build          cargo build --release, then .elf/.bin/.hex/.img in build/
#   make clean          remove build/ and target/
#   make rebuild        clean, then build
#   make lint           cargo clippy -D warnings (RGEN-007)
#   make fmt            cargo fmt
#   make flash          build, then flash build/rust-app.elf, verify and reset
#   make erase          erase the whole flash
#   make reset          reset the board
#   make debug          build, flash, then GDB halted at reset
#   make gdbserver      OpenOCD GDB server on :3333
#   make serial         serial console on the ST-LINK VCP (115200 8N1)
#   make probe          detect the ST-LINK and the target chip
#   make size           show the firmware size
#   make image          build the stm32-dev-rust Docker image
#   make help           show this help
#

ELF   := build/rust-app.elf
DEV   := scripts/docker-dev.sh
CARGO := $(DEV) run env CARGO_HOME=/work/.cargo-home cargo

.DEFAULT_GOAL := build

.PHONY: all build clean rebuild lint fmt flash erase reset debug gdbserver \
        serial probe size image help

all: build

build:
	scripts/build.sh

clean:
	rm -rf build target

rebuild: clean
	@$(MAKE) --no-print-directory build

lint:
	$(CARGO) clippy --release -- -D warnings

fmt:
	$(CARGO) fmt

flash: build
	$(DEV) flash $(ELF)

erase:
	$(DEV) erase

reset:
	$(DEV) reset

debug: build
	$(DEV) debug $(ELF) --load

gdbserver:
	$(DEV) gdbserver

serial:
	$(DEV) serial

probe:
	$(DEV) probe

size:
	@test -f $(ELF) || { echo "error: $(ELF) not found, run make build"; exit 1; }
	$(DEV) run arm-none-eabi-size /work/$(ELF)

image:
	$(DEV) build

help:
	@sed -n '/^#   make/p' $(firstword $(MAKEFILE_LIST)) | sed 's/^# \{0,1\}//'
