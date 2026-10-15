cd ../rawboot-rt
cargo clean
cd ../rawboot
cargo clean
@REM cargo build --release
cargo objcopy --bin rawboot --target armv7a-none-eabi --release -- -O binary rawboot.bin
@REM cargo objcopy --bin rawboot --target thumbv7em-none-eabihf --release -- -O binary rawboot.bin
@REM arm-none-eabi-objdump b:\RUST\AWDev\embed\rawboot\target\armv7a-none-eabi\release\rawboot binary -d
