cd ../rt
cargo clean
cd ../app
cargo clean
@REM cargo build --release
cargo objcopy --bin app --target armv7a-none-eabi --release -- -O binary app.bin
@REM cargo objcopy --bin app --target thumbv7em-none-eabihf --release -- -O binary app.bin
@REM arm-none-eabi-objdump b:\RUST\AWDev\embed\app\target\armv7a-none-eabi\release\app binary -d