/* Linker script for the SD/SPI loader image.

   BROM places a boot image at 0x00020000, so this variant must be linked there
   instead of the FEL address. Upstream uses a 128 KiB region at the same origin
   (`arch/arm32/mach-t113s3/link.ld`, `__RAM_BASE`), leaving room for the code
   and the pre-DRAM stack before the loader initialises DRAM. The section layout
   is shared with the FEL variant in `link-sections.x`. */
MEMORY
{
  RAM : ORIGIN = 0x00020000, LENGTH = 128k
}

INCLUDE link-sections.x
