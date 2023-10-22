K=../kernel

.PHONY: clean

mkfs: mkfs.c $K/fs.h $K/param.h
	gcc -Werror -Wall -I. -o mkfs mkfs.c

clean:
	rm -f mkfs
