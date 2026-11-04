/* The payload is a flat binary loaded at its image header's `load_addr`; this
   matches `ImageOptions::default` (`DEFAULT_LOAD_ADDR`). The entry point is the
   first byte of the image, so `entry == load_addr`. The loader hands off with a
   valid `SP`, so the payload needs no stack section of its own. */
MEMORY
{
  RAM : ORIGIN = 0x42000000, LENGTH = 128M
}

ENTRY(_start);

SECTIONS
{
  .start ORIGIN(RAM) :
  {
    KEEP(*(.payload.start));
  } > RAM

  .text :
  {
    *(.text .text.*);
  } > RAM

  .rodata :
  {
    *(.rodata .rodata.*);
  } > RAM

  .data :
  {
    *(.data .data.*);
  } > RAM

  .bss (NOLOAD) :
  {
    *(.bss .bss.*);
  } > RAM

  /DISCARD/ :
  {
    *(.ARM.exidx .ARM.exidx.*);
  }
}
