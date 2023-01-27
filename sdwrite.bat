FSUTIL file seteof app.bin 32768
checksum app.bin app.bin
dd if=app.bin of=\\?\Device\Harddisk3\Partition0 bs=1024 seek=8
pause