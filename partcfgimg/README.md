Generating the Partition Environment with partcfgimg
====================================================

`partcfgimg` is a tool written in Rust that generates an environment which is used to share partitioning information between bootloaders like [u-boot](https://docs.u-boot.org/en/latest/) or [barebox](https://www.barebox.org/), initramfs like [dracut](https://github.com/dracutdevs/dracut) or hypervisors like [L4re](https://www.st.com/en/partner-products-and-services/l4re-hypervisor.html). In order to make the adaption of these third party components as easy as possible, the environment is binary encoded following the [bincode](https://github.com/bincode-org/bincode) specification. To share a common base with the user space update tool and the linux system, `partcfgimg` relies on a partition configuration specified in JSON.

## Partition Description

In order to build and maintain a linux system, partitioning details have to be described and shared between the different stages and components of the system. The rupdate update tool uses a JSON formatted partition configuration in combination with a binary encoded partition environment to share information about:

- All persistent non-volatile storage devices that are capable of storing a filesystem
- Flash devices with and without controller
- Harddisks

The provided information is

- Independent of the access protocols and busses (USB, SD-Card, NVMe, etc.)
- Network file systems are out of scope (NFS, CIFS, sshfs, etc.)

This information can be used as configuration for the following components:

- Image build system (eg. bitbake, e2factory)
- Update tooling
- Bootloader (eg. u-boot or barebox)
- Initramfs
- Init system

### Partition Configuration (JSON)

The partition configuration, used by `partcfgimg` to generate the binary encoded partition environment, consists of a protocol version, a hash algorithm to be used for the binary partition environment and a list of partition sets.

| Name of Key    | Description                                                             |
|----------------|-------------------------------------------------------------------------|
| version        | Data structure syntax version                                           |
| hash_algorithm | Hash algorithm to be used along the binary representation               |
| partition_sets | List of partition sets                                                  |

#### Partition Sets

A partition set is a pair of partitions that are used for the same purpose in a pendulum update. So there is always one active partition used by the current system and one partition in which a new updated version could be written to. In order to support the system boot and update process, a partition set defines a name, mountpoint and a list of partitions along some optional fields like filesystem, size, user data, a comment or partition flags.

| Name of Key | Description                                                                |
|-------------|----------------------------------------------------------------------------|
| id          | Unique ID of the partition set (optional)                                  |
| name        | Unique name of the partition set                                           |
| filesystem  | Filesystem type to be used (optional)                                      |
| comment     | Describes the purpose of the set (optional)                                |
| size        | Size of the partition in bytes, null meaning remaining space (optional)    |
| mountpoint  | The directory this filesystem shall be mounted to.                         |
| user_data   | Machine readable data needed for partition handling                        |
| flags       | Flags to configure overlays, filesystem autodetect or encryption           |
| partitions  | List of partitions                                                         |

#### Partition Description

A partition consists of an optional variant, necessary if used as an updatable partition, and the information needed to access the partition from the linux system and the bootloader.


| Name of Key | Description                                                                |
|-------------|----------------------------------------------------------------------------|
| variant     | Either A or B                                                              |
| linux       | Partition information for the linux system                                 |
| bootloader  | Partition information for the bootloader                                   |

As mentioned before the linux and bootloader fields contain the necessary information to access the partitions from linux or the bootloader. It is distinguished into raw partitions

| Name of Key | Description                                                                |
|-------------|----------------------------------------------------------------------------|
| device      | Name of the device within the component                                    |
| offset      | Offset in bytes within the device                                          |

and formatted partitions:

| Name of Key | Description                                                                |
|-------------|----------------------------------------------------------------------------|
| device      | Name of the device within the component                                    |
| partition   | Name of the partition within the component                                 |

#### Partition Flags (optional)

A partition set can include specific partition flags to configure things like an overlayfs, partition encryption or if changes to this specific partition can be reverted by a rollback executed using the update tool.


| Name of Key | Description                                                                |
|-------------|----------------------------------------------------------------------------|
| CRYPTO_META | Cryptographic metadata appended to end of partition                        |
| AUTO_DETECT | The filesystem used can be determined using the partition content          |
| PART_META   | Partition meta data is provided as type byte (MBR) or type GUIDs (GPT)     |
| OVERLAY     | An overlayfs shall be mounted over to catch all writes.                    |
| MOUNT       | Automatically mount the corresponding partition                            |

#### Example Configuration

```javascript
{
    "version": "0.1.0",
    "hash_algorithm": "sha256",
    "partition_sets": [
        {
            "name": "boot_part",
            "comment": "Bootloader accessible partition layout",
            "partitions": [
                {
                    "linux": {
                        "device": "mmcblk0",
                        "offset": "0x2000"
                    },
                    "bootloader": {
                        "device": "0",
                        "offset": "0x2000"
                    }
                }
            ]
        },
        {
            "name": "update_env",
            "comment": "Shared update environment",
            "user_data": {
                "blob_offset": "0x1000"
            },
            "partitions": [
                {
                    "linux": {
                        "device": "mmcblk0",
                        "offset": "0x1000"
                    },
                    "bootloader": {
                        "device": "0",
                        "offset": "0x1000"
                    }
                }
            ]
        },
        {
            "id": 0,
            "name": "bootfs",
            "filesystem": "vfat",
            "mountpoint": "/boot",
            "flags": [
                "CRYPTO_META",
                "AUTO_DETECT"
            ],
            "partitions": [
                {
                    "variant": "A",
                    "linux": {
                        "device": "mmcblk0",
                        "partition": "p1"
                    },
                    "bootloader": {
                        "device": "0",
                        "partition": "0"
                    }
                },
                {
                    "variant": "B",
                    "linux": {
                        "device": "mmcblk0",
                        "partition": "p2"
                    },
                    "bootloader": {
                        "device": "0",
                        "partition": "1"
                    }
                }
            ]
        },
        {
            "id": 1,
            "name": "rootfs",
            "filesystem": "ext4",
            "mountpoint": "/",
            "flags": [
                "CRYPTO_META",
                "AUTO_DETECT"
            ],
            "partitions": [
                {
                    "variant": "A",
                    "linux": {
                        "device": "mmcblk0",
                        "partition": "p3"
                    }
                },
                {
                    "variant": "B",
                    "linux": {
                        "device": "mmcblk0",
                        "partition": "p5"
                    }
                }
            ]
        },
        {
            "name": "datafs",
            "filesystem": "zfs",
            "mountpoint": "/data",
            "comment": "Log and temp storage; not affected by updates",
            "flags": [
                "MOUNT"
            ],
            "partitions": [
                {
                    "linux": {
                        "device": "mmcblk0",
                        "partition": "p6"
                    }
                }
            ]
        },
    ]
}
```

**Partitions**

| Partitions on eMMC-device                               | dev node | mount point  |
|---------------------------------------------------------|----------|--------------|
| unpartitions area used for <br>bootloader and update env| mmcblk0  | (none)       |
| bootfs A (kernel + dtb)                                 | mmcblk0p1| ```/boot/``` |
| bootfs B (kernel + dtb)                                 | mmcblk0p2| ```/boot/``` |
| rootfs A - mount-point:                                 | mmcblk0p3| ```/```      |
| rootfs B - mount-point:                                 | mmcblk0p5| ```/```      |
| datafs - mount-point:                                   | mmcblk0p6| ```/data/``` |

**Hints:**

 1.  The partitions are used and mounted depending on the update-concept (either A or B, never both).
 2.  The datafs is not update-able in the sense of this concept, as it has no A and B variants.
 3.  This concept would allow to add a partition set for applications appfsA/appfsB if needed.


### Partition Environment (bincode)

The partition environment is a binary encoded (bincode) description of the current partition scheme, which major target is to make no or as little as possible assumptions on the bootloader or hypervisor. The main structure of this environment contains a magic, the current description format version, the number of partition sets, the partition sets, the number of partitions, the partition descriptions, a hashum type and a hashsum over the entire structure:

| Size    | Description            | Example       | Example Description                                    |
|---------|------------------------|---------------|--------------------------------------------------------|
| 4 Byte  | Magic Number           | "EBPC"        | short for EB Partition Config                          |
| 4 Byte  | Version                | 0x0000_0001   | Version 1                                              |
| 1 Byte  | Set Count              | 42            | Number of set descriptors                              |
| n Bytes | Set Descriptors        | see below     | Description of partition set                           |
| 1 Byte  | Partition Count        | 42            | Number of partition descriptors                        |
| n Bytes | Partitions Descriptors | see below     | n partitions descriptors                               |
| 4 Bytes | Checksum Identifier    | 13            | A numeric identifier for the used checksum type        |
| n Bytes | Checksum / signature   | &lt;SHA512&gt;| e.g. SHA512                                            |

#### Set Descriptors

A set descriptor describes a partition set using a unique name and unique ID:

| Size               | Description         | Example     | Example Description                                          |
|--------------------|---------------------|-------------|--------------------------------------------------------------|
| 1 Byte             | Numeric ID          | 3           | The numeric id of the partition set                          |
| 36 Byte            | String ID Set       | "rootfs"    | String identifier of the partition set                       |

#### Partition Descriptors

A partition descriptor contains all information on a single partition, like the device and partition name within the bootloader and within linux, along an optional reference to a partition set and a set variant. However, all partitions which are part of a partition set have to specify a set ID and a variant:

| Size               | Description         | Example     | Example Description                                          |
|--------------------|---------------------|-------------|--------------------------------------------------------------|
| 1 Byte             | Variant             | 0x00        | A = 0x00, B = 0x01                                           |
| 1 Byte             | Numeric Set         | 3           | Numeric ID of set                                            |
| 36 Byte            | Device ID           | "0"         | Bootloader device id (used by`mmc dev` or `fatload`)         |
| 36 Byte            | Partition ID        | "2"         | Bootloader partition id (used by`mmc dev` or `fatload`)      |
| 36 Byte            | Linux Device        | "mmcblk0"   | Linux device name or UUID                                    |
| 36 Byte            | Linux Partition     | "p0"        | Linux partition name or UUID                                 |

**Important:** 36 Byte are chosen to be able to use a UUID. (Not yet implemented)
