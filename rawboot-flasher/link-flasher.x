/* Linker script for the DRAM-linked flasher.

   The host writes the flat binary at `rawboot_image::FLASHER_LOAD_ADDR` after
   the DDR-init stub has made DRAM usable. `.text` is the first section, so the
   flat binary starts with `_start`; `.bss` is NOLOAD and zeroed on entry. The
   stack occupies the top of the reserved window and `_start` switches to it,
   then restores the BROM's stack before returning. No eGON header is present. */

MEMORY
{
  DRAM : ORIGIN = 0x40010000, LENGTH = 256k
}

ENTRY(_start)

SECTIONS
{
  .text ORIGIN(DRAM) :
  {
    KEEP(*(.text.entry))
    *(.text .text.*)
    *(.rodata .rodata.*)
    *(.data .data.*)
    . = ALIGN(4);
  } > DRAM

  .bss (NOLOAD) :
  {
    . = ALIGN(4);
    _sbss = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(4);
    _ebss = .;
  } > DRAM

  .stack (NOLOAD) :
  {
    . = ALIGN(8);
    _sstack = .;
    . = ORIGIN(DRAM) + LENGTH(DRAM);
    . = ALIGN(8);
    _estack = .;
  } > DRAM

  /DISCARD/ :
  {
    *(.ARM.exidx .ARM.exidx.*)
    *(.ARM.extab .ARM.extab.*)
    *(.ARM.attributes)
    *(.comment)
  }

  ASSERT(_estack - _sstack >= 0x4000, "flasher stack is too small")
}
