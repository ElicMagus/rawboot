/* Linker script for the FEL-callable DDR-init stub.

   The host writes the flat binary at `rawboot_image::DDR_STUB_LOAD_ADDR` and
   `exec`s it. The entry is the first byte of `.text`, so the flat binary starts
   with `_start`; `.bss` is NOLOAD and zeroed by `_start`. There is no eGON
   header, no stack section and no runtime: the BROM's stack is used and
   `_start` returns to the BROM. */

MEMORY
{
  SRAM : ORIGIN = 0x00028000, LENGTH = 20k
}

ENTRY(_start)

SECTIONS
{
  .text ORIGIN(SRAM) :
  {
    KEEP(*(.text.entry))
    *(.text .text.*)
    *(.rodata .rodata.*)
    *(.data .data.*)
    . = ALIGN(4);
  } > SRAM

  .bss (NOLOAD) :
  {
    . = ALIGN(4);
    _sbss = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(4);
    _ebss = .;
  } > SRAM

  /DISCARD/ :
  {
    *(.ARM.exidx .ARM.exidx.*)
    *(.ARM.extab .ARM.extab.*)
    *(.ARM.attributes)
    *(.comment)
  }

  ASSERT(_sbss >= ORIGIN(SRAM), "DDR-init stub BSS starts outside the SRAM window")
  ASSERT(_ebss <= ORIGIN(SRAM) + LENGTH(SRAM), "DDR-init stub exceeds its 20 KiB SRAM window")
  ASSERT(_ebss <= 0x0002d000, "DDR-init stub would overwrite the result block")
}
