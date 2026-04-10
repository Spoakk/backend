#include <stdlib.h>
#include "cubiomes/generator.h"

Generator* cubiomes_alloc_generator(void) {
    return (Generator*)calloc(1, sizeof(Generator));
}

void cubiomes_free_generator(Generator* g) {
    free(g);
}
