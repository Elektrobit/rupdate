# Assumptions of Use: The Bootloader


* The BL shall perform target-specific setup for the Linux Kernel to run with required features and performance.

* The BL shall read the update environment and the partition description as defined in []() to identify from which storage location to retrieve the Linux Kernel, initramfs, and -- if required by the Linux Kernel -- device tree.

* The BL shall write the update environment as defined.

* The BL shall load the Linux Kernel, initramfs, and optionally device-tree to the target-specific memory addresses as defined for the Linux Kernel.

* The BL shall pass the memory location of the initramfs and -- if present -- device tree to the Linux Kernel.

* The BL shall pass the location of the root file system via the Kernel command line to the Linux Kernel.

* The BL shall pass control to the Linux Kernel.
