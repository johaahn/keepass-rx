#ifndef __ASSEMBLER__
#define H_MALLOC_PREFIX
#include <malloc.h>
#include <stdalign.h>
#if !__GLIBC_PREREQ(2, 33)
struct mallinfo2 {
    size_t arena;
    size_t ordblks;
    size_t smblks;
    size_t hblks;
    size_t hblkhd;
    size_t usmblks;
    size_t fsmblks;
    size_t uordblks;
    size_t fordblks;
    size_t keepcost;
};
#endif
#undef N_ARENA
#define N_ARENA 1
#undef CONFIG_CLASS_REGION_SIZE
#define CONFIG_CLASS_REGION_SIZE 2147483648
#undef CONFIG_EXTENDED_SIZE_CLASSES
#define CONFIG_EXTENDED_SIZE_CLASSES true
#endif
