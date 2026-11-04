/*
 * noinit.x - reserved no-init window shared with BOOT (A-9)
 *
 * Author:  Sandesh Ghimire <sandesh@soccentric.com>
 * SRS:     RBT-001, RNFR-001; bootloader LAY-002 / BOOT_NOINIT_*
 *
 * Included after cortex-m-rt's link.x.  .noinit holds the fault record
 * and must survive a reset, so it is placed above BOOT's 60 KB DTCM
 * budget and above this application's own stack.  memory.x sets the
 * matching NOINIT region (top 1 KB of DTCM).
 */

SECTIONS
{
    .noinit (NOLOAD) : ALIGN(4)
    {
        . = ALIGN(4);
        __snoinit = .;
        KEEP(*(.noinit .noinit.*));
        . = ALIGN(4);
        __enoinit = .;
    } > NOINIT
}

ASSERT(ORIGIN(NOINIT) == 0x2001FC00 && LENGTH(NOINIT) == 1K,
       "A-9: NOINIT must match BOOT_NOINIT_ADDR/SIZE")
ASSERT(ORIGIN(NOINIT) >= 0x2000F000,
       "A-9: no-init window overlaps BOOT's 60 KB DTCM budget")
ASSERT(__snoinit >= ORIGIN(NOINIT),
       "A-9: .noinit starts outside the reserved window")
ASSERT(__enoinit <= ORIGIN(NOINIT) + LENGTH(NOINIT),
       "A-9: .noinit exceeds the reserved window")
