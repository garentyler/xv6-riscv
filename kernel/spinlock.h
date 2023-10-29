#include "types.h"
#pragma once

// Mutual exclusion lock.
struct spinlock {
  // Is the lock held?
  uint8 locked;
};

