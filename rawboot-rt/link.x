MEMORY
{  
  RAM : ORIGIN = 0x00028000, LENGTH = 64k
  /* RAM : ORIGIN = 0x00020000, LENGTH = 32k */
}

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

/* CHANGED! */
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