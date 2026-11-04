/* Linker script for the FEL loader image.

   `xfel write 0x00028000` places this image, so it is linked at that origin.
   The section layout is shared with the SD/SPI variant in `link-sections.x`;
   only the memory region differs. */
MEMORY
{
  RAM : ORIGIN = 0x00028000, LENGTH = 64k
}

INCLUDE link-sections.x
