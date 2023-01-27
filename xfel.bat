@REM xfel ddr t113-s3
@REM xfel write 0x40000000 xboot.bin
@REM xfel exec 0x40000000;
xfel write 0x00028000 app.bin
xfel exec 0x00028000;
pause