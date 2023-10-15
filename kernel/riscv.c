#include "kernel/riscv.h"

unsigned long rv_r_mhartid() {
    return r_mhartid();
}

unsigned int rv_r_tp() {
    return r_tp();
}

unsigned long rv_r_sstatus() {
    return r_sstatus();
}

void rv_w_sstatus(unsigned long x) {
    w_sstatus(x);
}

void rv_intr_on() {
    intr_on();
}

void rv_intr_off() {
    intr_off();
}

int rv_intr_get() {
    return intr_get();
}

