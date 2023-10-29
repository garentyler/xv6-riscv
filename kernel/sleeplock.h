#include "types.h"
#include "spinlock.h"
#pragma once

// Long-term locks for processes
struct sleeplock {
  // Is the lock held?
  uint8 locked;
};

