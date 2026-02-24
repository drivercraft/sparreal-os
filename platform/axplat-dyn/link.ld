/* 
 * Test linker script that works for both architectures
 * The actual OUTPUT_ARCH will be determined by the target
 */

INCLUDE "link.x"

SECTIONS {
    linkme_IRQ : { KEEP(*(linkme_IRQ)) }
    linkm2_IRQ : { KEEP(*(linkm2_IRQ)) }
    linkme_PAGE_FAULT : { KEEP(*(linkme_PAGE_FAULT)) }
    linkm2_PAGE_FAULT : { KEEP(*(linkm2_PAGE_FAULT)) }
    scope_local : { KEEP(*(scope_local)) }
}
INSERT AFTER .tbss;
