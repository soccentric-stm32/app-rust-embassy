/*
 * memory.x - RDEMO memory layout (runs under BOOT)
 *
 * Author:  Sandesh Ghimire <sandesh@soccentric.com>
 * SRS:     RBT-001; bootloader LAY-002 / APP-001
 *
 * The bootloader slot starts at bank sector 3. BOOT maps the active bank
 * at 0x08000000 (SWP_FB), so the image is always linked at 0x0800C000.
 * RAM: DTCM 128 KB + SRAM1 368 KB + SRAM2 16 KB are contiguous.
 */
MEMORY
{
    FLASH : ORIGIN = 0x0800C000, LENGTH = 976K
    RAM   : ORIGIN = 0x20000000, LENGTH = 512K
}

ASSERT(ORIGIN(FLASH) == 0x0800C000, "RBT-001: vector table must be at 0x0800C000");
