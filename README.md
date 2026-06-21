ROS kernel
==========

ROS is a kernel, written completely in rust.

--------------------------------------------

Progress
========
Currently, ROS is in it's early stage.
The kernel development started at June, 21st.

Contributing, issues, and pull requests
=======================================
Contributions are welcome.
Make sure, when contributing, to follow clean code
conducts, and document everything to make it easy to others.

Issues

If you find a bug, or noticed something you can't solve
by yourself, file an issue. We will try to review the issue
and solve it, if applicable.
make sure to add a "How to replicate" section, and list
what you've tried.

Pull requests

We accept pull requests. 
When committing your changes into your PR branch,
make sure to add a Signed-of-by.
Pull requests are going to be reviewed, and possibly,
merged.

Development
===========
You can clone this repo with 
`git clone https://github.com/SauloHS/ros`.
To run and open QEMU, run `cargo run`. To build
only, without opening QEMU, run `cargo build`.

Project tree
```text
ros |
    Cargo.toml
    Cargo.lock
    build.rs # build the kernel source code
    | src |
          main.rs # open QEMU with cargo run
    |
    | kernel |
             Cargo.toml
             | .cargo |
                      config.toml
             | src |
                   main.rs # main kernel entry
                   | drivers |
                             mod.rs
                             | video |
                                     framebuffer.rs # main video driver
                                     mod.rs

```

Last edited by Saulo Henrique at June, 21st.


