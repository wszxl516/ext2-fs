# ext2 FS bare metal driver


## Features
- File
  - read 
  - write (create) 
    - < 12kb 
- Dir
  - create 

## Toolchain
- rust

## Build & Run

```
$ cargo b 
```
```
$ dd if=/dev/zero of=hd.img bs=1M count=8
$ mkfs.ext2 hd.img
$ cargo r

```

## License

- MIT License
