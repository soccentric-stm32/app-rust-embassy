/*
 * memory.x - RDEMO memory layout (runs under BOOT)
 *
 * Author:  Sandesh Ghimire <sandesh@soccentric.com>
 * SRS:     RBT-001; bootloader LAY-002 / APP-001
 *
 * The bootloader slot starts at bank sector 3. BOOT maps the active bank
 * at 0x08000000 (SWP_FB), so the image is always linked at 0x0800C000.
 * RAM: DTCM only.  The top 1 KB of DTCM is the no-init window shared
 * with BOOT (BOOT_NOINIT_ADDR); the main stack starts just below it, so
 * a fault record there survives a reset and BOOT never clears it (A-9).
 */
MEMORY
{
    FLASH  : ORIGIN = 0x0800C000, LENGTH = 976K
    RAM    : ORIGIN = 0x20000000, LENGTH = 127K
    NOINIT : ORIGIN = 0x2001FC00, LENGTH = 1K
}

ASSERT(ORIGIN(FLASH) == 0x0800C000, "RBT-001: vector table must be at 0x0800C000");
