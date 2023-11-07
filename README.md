# xv6-riscv

MIT's xv6-riscv operating system, now in Rust!

This is a passion project for me - I've always wanted to write an operating system.
I decided to port the xv6 operating system so that I could try porting a moderately
sized codebase to my favorite programming language, Rust.

> xv6 is a re-implementation of Dennis Ritchie's and Ken Thompson's Unix
> Version 6 (v6). xv6 loosely follows the structure and style of v6,
> but is implemented for a modern RISC-V multiprocessor using ANSI C.

To start the project, I made a basic Rust crate that compiled to a static library.
At link time, the linker includes the static library into the final binary to result
in a hybrid kernel. When the entire kernel is written in Rust, the link process should
be a lot simpler (just Rust and assembly). At that point, I can start refactoring the
kernel to use more of Rust's features that don't translate well across FFI boundaries.

## Features

- [x] Multi-core processing
- [x] Paging
- [x] Pre-emptive multitasking
- [x] File system
- [x] Process communication using pipes
- [ ] Entirely Rust kernel (no more C code)
- [ ] [Round-robin scheduling](https://en.wikipedia.org/wiki/Round-robin_scheduling)
- [ ] Rust ABI for syscalls (I'll probably use [stabby](https://crates.io/crates/stabby) for this)
- [ ] Networking
- [ ] Running on real hardware (likely a [Milk-V Duo](https://milkv.io/duo))
- [ ] Port Rust standard library

## Building and running

Build requirements:

- [A RISC-V C toolchain](https://github.com/riscv/riscv-gnu-toolchain)
- [QEMU](https://www.qemu.org/download/) (qemu-system-riscv64)
- [A nightly Rust toolchain](https://rustup.rs/)

The makefile is split into multiple levels to clearly separate scripts,
but most important commands can be run from the project root.

- `make kernel` builds the kernel.
- `make mkfs` builds `mkfs`, the tool to help create the file system image.
- `make fs.img` uses `mkfs` to build the file system image.
- `make qemu` builds the kernel and file system, and then runs it in QEMU.
- `make clean` removes built artifacts, including from Rust.

## Contributing

Pull requests will be ignored.

## Authors and acknowledgements

Rewrite:

- Garen Tyler \<<garentyler@garen.dev>>

Source:

> xv6 is inspired by John Lions's Commentary on UNIX 6th Edition (Peer
> to Peer Communications; ISBN: 1-57398-013-7; 1st edition (June 14,
> 2000)).  See also https://pdos.csail.mit.edu/6.1810/, which provides
> pointers to on-line resources for v6.
> 
> The following people have made contributions: Russ Cox (context switching,
> locking), Cliff Frey (MP), Xiao Yu (MP), Nickolai Zeldovich, and Austin
> Clements.
> 
> We are also grateful for the bug reports and patches contributed by
> Takahiro Aoyagi, Silas Boyd-Wickizer, Anton Burtsev, carlclone, Ian
> Chen, Dan Cross, Cody Cutler, Mike CAT, Tej Chajed, Asami Doi,
> eyalz800, Nelson Elhage, Saar Ettinger, Alice Ferrazzi, Nathaniel
> Filardo, flespark, Peter Froehlich, Yakir Goaron, Shivam Handa, Matt
> Harvey, Bryan Henry, jaichenhengjie, Jim Huang, Matúš Jókay, John
> Jolly, Alexander Kapshuk, Anders Kaseorg, kehao95, Wolfgang Keller,
> Jungwoo Kim, Jonathan Kimmitt, Eddie Kohler, Vadim Kolontsov, Austin
> Liew, l0stman, Pavan Maddamsetti, Imbar Marinescu, Yandong Mao, Matan
> Shabtay, Hitoshi Mitake, Carmi Merimovich, Mark Morrissey, mtasm, Joel
> Nider, Hayato Ohhashi, OptimisticSide, Harry Porter, Greg Price, Jude
> Rich, segfault, Ayan Shafqat, Eldar Sehayek, Yongming Shen, Fumiya
> Shigemitsu, Cam Tenny, tyfkda, Warren Toomey, Stephen Tu, Rafael Ubal,
> Amane Uehara, Pablo Ventura, Xi Wang, WaheedHafez, Keiichi Watanabe,
> Nicolas Wolovick, wxdao, Grant Wu, Jindong Zhang, Icenowy Zheng,
> ZhUyU1997, and Zou Chang Wei.

## License

All code written by me in this project is [LGPLv3](https://choosealicense.com/licenses/lgpl-3.0/) licensed.
Any existing code appears to be under the [MIT](https://choosealicense.com/licenses/mit/) license.
