Update Tool
===========

`rupdate` is a tool written in Rust that implements a basic pendulum update. Basic in this case means that it is only able to update partitions by writing new partition images. However, `rupdate` is able to update only a subset of the available partitions and to recover from power shortages, sudden reboots or system crashes during the update process. Furthermore it is part of a larger update concept, which also defines a [partition configuration](../partcfgimg/) that is used by the update tool to gather all necessary information about the system's partitioning scheme.

## Manual

[manual](manual.txt)
