/* Section layout shared by the FEL and SD/SPI loader images.

   Included by `link.x` (FEL, 0x00028000) and `link-boot.x` (SD/SPI, 0x00020000)
   after their MEMORY blocks, so the two variants cannot drift apart. */

/* The entry point is the reset handler */
ENTRY(_start);

SECTIONS
{

  .head ORIGIN(RAM):
  {
      PROVIDE(__spl_start = .);
    KEEP(*(.head.head));
    KEEP(*(.head.start));
  } > RAM

  .text :
  {
    PROVIDE(__text_start = .);
    *(.text .text.*);

    PROVIDE(__text_end = .);
  } > RAM

  .rodata :
  {
    *(.rodata .rodata.*);
  } > RAM

  .bss :
  {
    _sbss = .;
    *(.bss .bss.*);
    _ebss = .;
  } > RAM

   .data : AT(ADDR(.rodata) + SIZEOF(.rodata))
  {
    _sdata = .;
    *(.data .data.*);
    _edata = .;
  } > RAM

  _sidata = LOADADDR(.data);
 PROVIDE(__spl_end = .);
 PROVIDE(__spl_size = __spl_end - __spl_start);

  /* The spec keeps the loader at 64 KiB or smaller in SRAM; the boot variant's
     128 KiB region is for the stack, not for a larger image. */
  ASSERT(__spl_size <= 64K, "loader exceeds the 64 KiB SRAM budget")

  /* stack section */
    .stack (NOLOAD):
    {
        . = ALIGN(8);
        _sstack = .;
        . = ORIGIN(RAM) + LENGTH(RAM);
        . = ALIGN(8);
        _estack = .;
    } > RAM

  ASSERT(_estack - _sstack >= 0x1c00, "SRAM stack too small for pre-DRAM bring-up")

  /DISCARD/ :
  {
    *(.ARM.exidx .ARM.exidx.*);
  }
}
